//! Cancellation-flag registry for ralph-loop'd agents. Each entry maps the
//! currently-running iteration's PID to a cancel flag the loop controller
//! polls between iterations. The kill handler trips the flag right before
//! signalling the process group so the next iteration doesn't fire.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;

#[derive(Clone, Default)]
pub struct LoopRegistry {
    by_pid: Arc<Mutex<HashMap<u32, Arc<AtomicBool>>>>,
}

impl LoopRegistry {
    pub fn register(&self, pid: u32, flag: Arc<AtomicBool>) {
        self.by_pid.lock().insert(pid, flag);
    }

    pub fn deregister(&self, pid: u32) {
        self.by_pid.lock().remove(&pid);
    }

    /// Atomically move a task's registration from one iteration's PID to
    /// the next. The cancel flag is carried forward, so a kill that lands
    /// during the inter-iteration gap still stops the loop.
    pub fn advance(&self, from: u32, to: u32) {
        let mut by_pid = self.by_pid.lock();
        if let Some(flag) = by_pid.remove(&from) {
            by_pid.insert(to, flag);
        }
    }

    /// Trip the cancel flag for `pid` if it's a known looping iteration.
    /// Returns `true` when a flag was found and set.
    pub fn cancel(&self, pid: u32) -> bool {
        if let Some(flag) = self.by_pid.lock().get(&pid).cloned() {
            flag.store(true, Ordering::SeqCst);
            true
        } else {
            false
        }
    }

    /// True when `pid` belongs to a multi-iteration agent whose loop
    /// controller is driving + finalizing the run. The central reaper
    /// skips these pids so the two paths don't race.
    pub fn is_registered(&self, pid: u32) -> bool {
        self.by_pid.lock().contains_key(&pid)
    }
}

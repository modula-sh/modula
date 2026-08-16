/// Windows console-ctrl handler: registers a callback for CTRL_C, CTRL_BREAK,
/// and CTRL_CLOSE events so the engine can run shutdown cleanup from a signal
/// just as SIGTERM/SIGINT do on Unix. Compiles only on Windows.
#[cfg(windows)]
mod windows_impl {
    use std::io;
    use std::sync::OnceLock;

    use windows_sys::Win32::System::Console::{
        SetConsoleCtrlHandler, CTRL_BREAK_EVENT, CTRL_CLOSE_EVENT, CTRL_C_EVENT,
    };

    // Only one ctrl handler is needed per process; store the callback globally.
    static HANDLER: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

    /// Registers `f` as the console-ctrl callback. Subsequent CTRL_C, CTRL_BREAK,
    /// and CTRL_CLOSE events invoke `f` on an OS-created thread.
    ///
    /// The first call wins; later calls leave the original callback in place.
    /// Intended to be called once before the engine serves.
    pub fn set_ctrl_handler(f: impl Fn() + Send + Sync + 'static) -> io::Result<()> {
        let _ = HANDLER.set(Box::new(f));
        if unsafe { SetConsoleCtrlHandler(Some(ctrl_dispatch), 1) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    extern "system" fn ctrl_dispatch(ctrl_type: u32) -> i32 {
        if matches!(
            ctrl_type,
            CTRL_C_EVENT | CTRL_BREAK_EVENT | CTRL_CLOSE_EVENT
        ) {
            if let Some(f) = HANDLER.get() {
                f();
            }
            return 1; // handled
        }
        0 // pass to next handler
    }
}

#[cfg(windows)]
pub use windows_impl::set_ctrl_handler;

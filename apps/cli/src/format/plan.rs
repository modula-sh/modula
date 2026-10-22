//! The one-line plan progress shown by `variant get`, `task get`, and plan writes.

use modula_types::PlanProgress;

/// `5/9 tasks · phase 2/4 Engine runtime · task 2.3 Wire handler`; finished
/// (done or skipped) over total, then the current phase and task if any.
pub fn plan_progress(p: &PlanProgress) -> String {
    let mut line = format!("{}/{} tasks", p.tasks_done + p.tasks_skipped, p.tasks_total);
    if let (Some(id), Some(title)) = (&p.current_phase_id, &p.current_phase_title) {
        line.push_str(&format!(" · phase {id}/{} {title}", p.phases_total));
    }
    if let (Some(id), Some(title)) = (&p.current_task_id, &p.current_task_title) {
        line.push_str(&format!(" · task {id} {title}"));
    }
    line
}

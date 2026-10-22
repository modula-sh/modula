//! A variant's plan: one JSON document the Researcher writes whole and the
//! Worker advances through [`PlanPatch`]es. The engine is the only parser, so
//! every rule lives here and a bad document fails at write time.
//!
//! Phase and task ids are positional (`"2"`, `"2.3"`). [`Plan::parse`]
//! assigns them and every append mints the next one, so they stay positional.

use std::fmt::Write;
use std::str::FromStr;

use modula_rpc::v1 as pb;
use modula_rpc::v1::patch_plan_request::Op;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub version: u32,
    pub title: String,
    pub problem: String,
    pub approach: String,
    pub projects: Vec<PlanProject>,
    pub phases: Vec<PlanPhase>,
    pub risks: String,
    pub test_plan: String,
    #[serde(default)]
    pub summaries: Vec<PlanSummary>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanProject {
    pub name: String,
    pub changes: Vec<PlanChange>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanChange {
    pub path: String,
    pub change: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanPhase {
    #[serde(default)]
    pub id: String,
    pub title: String,
    pub scope: String,
    pub approach: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub deviations: String,
    pub tasks: Vec<PlanTask>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanTask {
    #[serde(default)]
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub status: PlanTaskStatus,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanTaskStatus {
    #[default]
    Pending,
    InProgress,
    Done,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanSummary {
    pub kind: PlanSummaryKind,
    #[serde(default)]
    pub round: Option<u32>,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanSummaryKind {
    Done,
    Rework,
}

/// Derived on read so it never drifts from the document. The current task is
/// the `in_progress` one, else the first `pending` one; both current fields
/// are `None` once every task is `done` or `skipped`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PlanProgress {
    pub tasks_total: u32,
    pub tasks_done: u32,
    pub tasks_skipped: u32,
    pub current_phase_id: Option<String>,
    pub current_phase_title: Option<String>,
    pub current_task_id: Option<String>,
    pub current_task_title: Option<String>,
}

/// The Worker's edits. Each is append-only or a status change, so no patch
/// can rewrite the Researcher's structure or prose.
#[derive(Debug, Clone, PartialEq)]
pub enum PlanPatch {
    SetTaskStatus {
        task: String,
        status: PlanTaskStatus,
        note: Option<String>,
    },
    AddTask {
        phase: String,
        title: String,
    },
    AddPhase {
        title: String,
        scope: String,
        approach: String,
        notes: String,
        tasks: Vec<String>,
    },
    AppendDeviations {
        phase: String,
        content: String,
    },
    AddSummary(PlanSummary),
}

impl Plan {
    /// Parse and validate a document, overwriting any supplied ids.
    pub fn parse(text: &str) -> Result<Plan, String> {
        let mut plan: Plan =
            serde_json::from_str(text).map_err(|e| format!("invalid plan: {e}"))?;
        if plan.version != 1 {
            return Err(format!(
                "unsupported plan version {} (expected 1)",
                plan.version
            ));
        }
        if plan.phases.is_empty() {
            return Err("plan has no phases".into());
        }
        for (i, phase) in plan.phases.iter_mut().enumerate() {
            phase.id = (i + 1).to_string();
            for (j, task) in phase.tasks.iter_mut().enumerate() {
                task.id = format!("{}.{}", phase.id, j + 1);
            }
        }
        for phase in &plan.phases {
            check_phase(phase)?;
        }
        let in_progress: Vec<&str> = plan
            .tasks()
            .filter(|(_, t)| t.status == PlanTaskStatus::InProgress)
            .map(|(_, t)| t.id.as_str())
            .collect();
        if in_progress.len() > 1 {
            return Err(format!(
                "tasks {} are in_progress; at most one task may be in_progress",
                in_progress.join(", ")
            ));
        }
        for summary in &plan.summaries {
            check_summary(summary)?;
        }
        Ok(plan)
    }

    fn tasks(&self) -> impl Iterator<Item = (&PlanPhase, &PlanTask)> {
        self.phases
            .iter()
            .flat_map(|p| p.tasks.iter().map(move |t| (p, t)))
    }

    pub fn progress(&self) -> PlanProgress {
        let mut progress = PlanProgress::default();
        for (_, task) in self.tasks() {
            progress.tasks_total += 1;
            match task.status {
                PlanTaskStatus::Done => progress.tasks_done += 1,
                PlanTaskStatus::Skipped => progress.tasks_skipped += 1,
                _ => {}
            }
        }
        let current = self
            .tasks()
            .find(|(_, t)| t.status == PlanTaskStatus::InProgress)
            .or_else(|| {
                self.tasks()
                    .find(|(_, t)| t.status == PlanTaskStatus::Pending)
            });
        if let Some((phase, task)) = current {
            progress.current_phase_id = Some(phase.id.clone());
            progress.current_phase_title = Some(phase.title.clone());
            progress.current_task_id = Some(task.id.clone());
            progress.current_task_title = Some(task.title.clone());
        }
        progress
    }

    /// Apply one patch. Returns the id of the task, phase, or summary (1-based
    /// index) it touched or created.
    pub fn apply(&mut self, patch: PlanPatch) -> Result<String, String> {
        match patch {
            PlanPatch::SetTaskStatus { task, status, note } => {
                let other_in_progress = self
                    .tasks()
                    .find(|(_, t)| t.status == PlanTaskStatus::InProgress && t.id != task)
                    .map(|(_, t)| t.id.clone());
                let t = self
                    .phases
                    .iter_mut()
                    .flat_map(|p| p.tasks.iter_mut())
                    .find(|t| t.id == task)
                    .ok_or_else(|| format!("unknown task {task}"))?;
                match status {
                    PlanTaskStatus::InProgress => {
                        if let Some(other) = other_in_progress {
                            return Err(format!(
                                "task {other} is in_progress; mark it done, skipped, or pending first"
                            ));
                        }
                    }
                    PlanTaskStatus::Done if t.status != PlanTaskStatus::InProgress => {
                        return Err(format!(
                            "task {task} is {}; mark it in_progress before done",
                            t.status.as_str()
                        ));
                    }
                    PlanTaskStatus::Skipped => {
                        if !matches!(
                            t.status,
                            PlanTaskStatus::Pending | PlanTaskStatus::InProgress
                        ) {
                            return Err(format!(
                                "task {task} is {}; only pending or in_progress tasks can be skipped",
                                t.status.as_str()
                            ));
                        }
                        if note.as_deref().is_none_or(is_blank) {
                            return Err(format!("task {task}: skipped needs a note"));
                        }
                    }
                    _ => {}
                }
                t.status = status;
                if note.is_some() {
                    t.note = note;
                }
                Ok(task)
            }
            PlanPatch::AddTask { phase, title } => {
                let p = self.phase_mut(&phase)?;
                let id = format!("{}.{}", p.id, p.tasks.len() + 1);
                if is_blank(&title) {
                    return Err(format!("task {id}: title is blank"));
                }
                p.tasks.push(PlanTask {
                    id: id.clone(),
                    title,
                    status: PlanTaskStatus::Pending,
                    note: None,
                });
                Ok(id)
            }
            PlanPatch::AddPhase {
                title,
                scope,
                approach,
                notes,
                tasks,
            } => {
                let id = (self.phases.len() + 1).to_string();
                let phase = PlanPhase {
                    tasks: tasks
                        .into_iter()
                        .enumerate()
                        .map(|(j, title)| PlanTask {
                            id: format!("{id}.{}", j + 1),
                            title,
                            status: PlanTaskStatus::Pending,
                            note: None,
                        })
                        .collect(),
                    id: id.clone(),
                    title,
                    scope,
                    approach,
                    notes,
                    deviations: String::new(),
                };
                check_phase(&phase)?;
                self.phases.push(phase);
                Ok(id)
            }
            PlanPatch::AppendDeviations { phase, content } => {
                let p = self.phase_mut(&phase)?;
                if is_blank(&content) {
                    return Err(format!("phase {phase}: deviations are blank"));
                }
                if !p.deviations.is_empty() {
                    p.deviations.push_str("\n\n");
                }
                p.deviations.push_str(&content);
                Ok(phase)
            }
            PlanPatch::AddSummary(summary) => {
                check_summary(&summary)?;
                self.summaries.push(summary);
                Ok(self.summaries.len().to_string())
            }
        }
    }

    fn phase_mut(&mut self, id: &str) -> Result<&mut PlanPhase, String> {
        self.phases
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| format!("unknown phase {id}"))
    }

    /// The reviewer view: the same sections the markdown spec folder had, with
    /// a checklist per phase (`[~]` in progress, `[-]` skipped).
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        let _ = writeln!(md, "# {}\n", self.title);
        let _ = writeln!(md, "## Problem\n{}\n", self.problem.trim_end());
        let _ = writeln!(md, "## Approach\n{}\n", self.approach.trim_end());
        let _ = writeln!(md, "## Projects touched");
        for project in &self.projects {
            let _ = writeln!(md, "- project: {}", project.name);
            for c in &project.changes {
                let _ = writeln!(md, "  - `{}`: {}", c.path, c.change);
            }
        }
        for phase in &self.phases {
            let _ = writeln!(md, "\n## Phase {}: {}\n", phase.id, phase.title);
            let _ = writeln!(md, "### Scope\n{}\n", phase.scope.trim_end());
            let _ = writeln!(md, "### Approach\n{}\n", phase.approach.trim_end());
            if !phase.notes.is_empty() {
                let _ = writeln!(md, "### Notes\n{}\n", phase.notes.trim_end());
            }
            if !phase.deviations.is_empty() {
                let _ = writeln!(md, "### Deviations\n{}\n", phase.deviations.trim_end());
            }
            let _ = writeln!(md, "### Tasks");
            for task in &phase.tasks {
                let mark = match task.status {
                    PlanTaskStatus::Pending => ' ',
                    PlanTaskStatus::InProgress => '~',
                    PlanTaskStatus::Done => 'x',
                    PlanTaskStatus::Skipped => '-',
                };
                let _ = write!(md, "- [{mark}] {} {}", task.id, task.title);
                match (&task.note, task.status) {
                    (Some(note), PlanTaskStatus::Skipped) => {
                        let _ = write!(md, " (skipped: {note})");
                    }
                    (Some(note), _) => {
                        let _ = write!(md, " (note: {note})");
                    }
                    _ => {}
                }
                md.push('\n');
            }
        }
        let _ = writeln!(md, "\n## Risks & tradeoffs\n{}\n", self.risks.trim_end());
        let _ = writeln!(md, "## Test plan\n{}", self.test_plan.trim_end());
        for summary in &self.summaries {
            match (summary.kind, summary.round) {
                (PlanSummaryKind::Rework, Some(round)) => {
                    let _ = writeln!(md, "\n## Rework round {round}");
                }
                _ => {
                    let _ = writeln!(md, "\n## Done");
                }
            }
            let _ = writeln!(md, "{}", summary.content.trim_end());
        }
        md
    }
}

fn is_blank(s: &str) -> bool {
    s.trim().is_empty()
}

impl PlanTaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            PlanTaskStatus::Pending => "pending",
            PlanTaskStatus::InProgress => "in_progress",
            PlanTaskStatus::Done => "done",
            PlanTaskStatus::Skipped => "skipped",
        }
    }
}

/// Parse a wire string with serde's message ("unknown variant `x`, expected
/// one of …"), the same one a bad document gets.
fn from_wire<T: serde::de::DeserializeOwned>(s: &str) -> Result<T, String> {
    serde_json::from_value(serde_json::Value::String(s.to_string())).map_err(|e| e.to_string())
}

impl FromStr for PlanTaskStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        from_wire(s)
    }
}

impl FromStr for PlanSummaryKind {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        from_wire(s)
    }
}

impl PlanSummaryKind {
    pub fn as_str(self) -> &'static str {
        match self {
            PlanSummaryKind::Done => "done",
            PlanSummaryKind::Rework => "rework",
        }
    }
}

impl From<pb::PlanProgress> for PlanProgress {
    fn from(p: pb::PlanProgress) -> Self {
        Self {
            tasks_total: p.tasks_total,
            tasks_done: p.tasks_done,
            tasks_skipped: p.tasks_skipped,
            current_phase_id: p.current_phase_id,
            current_phase_title: p.current_phase_title,
            current_task_id: p.current_task_id,
            current_task_title: p.current_task_title,
        }
    }
}

impl From<PlanProgress> for pb::PlanProgress {
    fn from(p: PlanProgress) -> Self {
        Self {
            tasks_total: p.tasks_total,
            tasks_done: p.tasks_done,
            tasks_skipped: p.tasks_skipped,
            current_phase_id: p.current_phase_id,
            current_phase_title: p.current_phase_title,
            current_task_id: p.current_task_id,
            current_task_title: p.current_task_title,
        }
    }
}

impl TryFrom<Op> for PlanPatch {
    type Error = String;
    fn try_from(op: Op) -> Result<Self, String> {
        Ok(match op {
            Op::SetTaskStatus(o) => PlanPatch::SetTaskStatus {
                task: o.task,
                status: o.status.parse()?,
                note: o.note,
            },
            Op::AddTask(o) => PlanPatch::AddTask {
                phase: o.phase,
                title: o.title,
            },
            Op::AddPhase(o) => PlanPatch::AddPhase {
                title: o.title,
                scope: o.scope,
                approach: o.approach,
                notes: o.notes,
                tasks: o.tasks,
            },
            Op::AppendDeviations(o) => PlanPatch::AppendDeviations {
                phase: o.phase,
                content: o.content,
            },
            Op::AddSummary(o) => PlanPatch::AddSummary(PlanSummary {
                kind: o.kind.parse()?,
                round: o.round,
                content: o.content,
            }),
        })
    }
}

impl From<PlanPatch> for Op {
    fn from(p: PlanPatch) -> Self {
        match p {
            PlanPatch::SetTaskStatus { task, status, note } => {
                Op::SetTaskStatus(pb::PlanSetTaskStatus {
                    task,
                    status: status.as_str().into(),
                    note,
                })
            }
            PlanPatch::AddTask { phase, title } => Op::AddTask(pb::PlanAddTask { phase, title }),
            PlanPatch::AddPhase {
                title,
                scope,
                approach,
                notes,
                tasks,
            } => Op::AddPhase(pb::PlanAddPhase {
                title,
                scope,
                approach,
                notes,
                tasks,
            }),
            PlanPatch::AppendDeviations { phase, content } => {
                Op::AppendDeviations(pb::PlanAppendDeviations { phase, content })
            }
            PlanPatch::AddSummary(s) => Op::AddSummary(pb::PlanSummary {
                kind: s.kind.as_str().into(),
                round: s.round,
                content: s.content,
            }),
        }
    }
}

fn check_phase(phase: &PlanPhase) -> Result<(), String> {
    if is_blank(&phase.title) {
        return Err(format!("phase {}: title is blank", phase.id));
    }
    if phase.tasks.is_empty() {
        return Err(format!("phase {}: has no tasks", phase.id));
    }
    for task in &phase.tasks {
        if is_blank(&task.title) {
            return Err(format!("task {}: title is blank", task.id));
        }
        if task.status == PlanTaskStatus::Skipped && task.note.as_deref().is_none_or(is_blank) {
            return Err(format!("task {}: skipped needs a note", task.id));
        }
    }
    Ok(())
}

fn check_summary(summary: &PlanSummary) -> Result<(), String> {
    match (summary.kind, summary.round) {
        (PlanSummaryKind::Rework, None | Some(0)) => {
            Err("rework summary needs a round >= 1".into())
        }
        (PlanSummaryKind::Done, Some(_)) => Err("done summary takes no round".into()),
        _ if is_blank(&summary.content) => Err("summary content is blank".into()),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn doc() -> serde_json::Value {
        json!({
            "version": 1,
            "title": "MOD-1 Thing",
            "problem": "p",
            "approach": "a",
            "projects": [{"name": "Modula", "changes": [{"path": "x.rs", "change": "new"}]}],
            "phases": [
                {"id": "9", "title": "One", "scope": "s", "approach": "a",
                 "tasks": [{"id": "z", "title": "A"}, {"title": "B"}]},
                {"title": "Two", "scope": "s", "approach": "a", "notes": "n",
                 "tasks": [{"title": "C"}]}
            ],
            "risks": "r",
            "test_plan": "t"
        })
    }

    fn plan() -> Plan {
        Plan::parse(&doc().to_string()).unwrap()
    }

    fn err(v: serde_json::Value) -> String {
        Plan::parse(&v.to_string()).unwrap_err()
    }

    fn set(plan: &mut Plan, task: &str, status: PlanTaskStatus) -> Result<String, String> {
        plan.apply(PlanPatch::SetTaskStatus {
            task: task.into(),
            status,
            note: None,
        })
    }

    #[test]
    fn parse_assigns_positional_ids() {
        let p = plan();
        assert_eq!(p.phases[0].id, "1");
        assert_eq!(p.phases[1].id, "2");
        let ids: Vec<_> = p.tasks().map(|(_, t)| t.id.as_str()).collect();
        assert_eq!(ids, ["1.1", "1.2", "2.1"]);
        assert_eq!(p.phases[0].tasks[0].status, PlanTaskStatus::Pending);
        assert!(p.summaries.is_empty());
    }

    #[test]
    fn parse_round_trips_through_serialization() {
        let p = plan();
        assert_eq!(p, Plan::parse(&serde_json::to_string(&p).unwrap()).unwrap());
    }

    #[test]
    fn parse_rejects_unknown_field() {
        let mut v = doc();
        v["phases"][0]["scop"] = json!("x");
        let e = err(v);
        assert!(e.contains("unknown field `scop`"), "{e}");
        assert!(e.contains("expected one of"), "{e}");
    }

    #[test]
    fn parse_rejects_missing_field_and_bad_version() {
        let mut v = doc();
        v.as_object_mut().unwrap().remove("risks");
        assert!(err(v).contains("missing field `risks`"));
        let mut v = doc();
        v["version"] = json!(2);
        assert_eq!(err(v), "unsupported plan version 2 (expected 1)");
    }

    #[test]
    fn parse_rejects_rule_violations() {
        let mut v = doc();
        v["phases"] = json!([]);
        assert_eq!(err(v), "plan has no phases");

        let mut v = doc();
        v["phases"][1]["tasks"] = json!([]);
        assert_eq!(err(v), "phase 2: has no tasks");

        let mut v = doc();
        v["phases"][0]["title"] = json!("  ");
        assert_eq!(err(v), "phase 1: title is blank");

        let mut v = doc();
        v["phases"][0]["tasks"][1]["title"] = json!("");
        assert_eq!(err(v), "task 1.2: title is blank");

        let mut v = doc();
        v["phases"][1]["tasks"][0]["status"] = json!("skipped");
        assert_eq!(err(v), "task 2.1: skipped needs a note");

        let mut v = doc();
        v["phases"][0]["tasks"][0]["status"] = json!("in_progress");
        v["phases"][1]["tasks"][0]["status"] = json!("in_progress");
        assert_eq!(
            err(v),
            "tasks 1.1, 2.1 are in_progress; at most one task may be in_progress"
        );

        let mut v = doc();
        v["phases"][0]["tasks"][0]["status"] = json!("blocked");
        assert!(err(v).contains("unknown variant `blocked`"));

        let mut v = doc();
        v["summaries"] = json!([{"kind": "rework", "content": "c"}]);
        assert_eq!(err(v), "rework summary needs a round >= 1");
    }

    #[test]
    fn task_status_transitions() {
        let mut p = plan();
        assert!(set(&mut p, "1.1", PlanTaskStatus::Done)
            .unwrap_err()
            .contains("is pending; mark it in_progress before done"));
        assert_eq!(
            set(&mut p, "1.1", PlanTaskStatus::InProgress).unwrap(),
            "1.1"
        );
        assert_eq!(
            set(&mut p, "1.2", PlanTaskStatus::InProgress).unwrap_err(),
            "task 1.1 is in_progress; mark it done, skipped, or pending first"
        );
        set(&mut p, "1.1", PlanTaskStatus::Done).unwrap();
        assert!(set(&mut p, "1.1", PlanTaskStatus::Done)
            .unwrap_err()
            .contains("is done"));
        assert!(set(&mut p, "1.1", PlanTaskStatus::Skipped)
            .unwrap_err()
            .contains("only pending or in_progress"));
        assert_eq!(
            set(&mut p, "1.2", PlanTaskStatus::Skipped).unwrap_err(),
            "task 1.2: skipped needs a note"
        );
        p.apply(PlanPatch::SetTaskStatus {
            task: "1.2".into(),
            status: PlanTaskStatus::Skipped,
            note: Some("not needed".into()),
        })
        .unwrap();
        assert_eq!(p.phases[0].tasks[1].note.as_deref(), Some("not needed"));
        set(&mut p, "1.1", PlanTaskStatus::Pending).unwrap();
        assert_eq!(p.phases[0].tasks[0].status, PlanTaskStatus::Pending);
        assert_eq!(
            set(&mut p, "7.1", PlanTaskStatus::InProgress).unwrap_err(),
            "unknown task 7.1"
        );
    }

    #[test]
    fn appends_mint_ids() {
        let mut p = plan();
        let id = p
            .apply(PlanPatch::AddTask {
                phase: "1".into(),
                title: "D".into(),
            })
            .unwrap();
        assert_eq!(id, "1.3");
        assert_eq!(
            p.apply(PlanPatch::AddTask {
                phase: "5".into(),
                title: "D".into()
            })
            .unwrap_err(),
            "unknown phase 5"
        );

        let add_phase = |tasks: Vec<&str>| PlanPatch::AddPhase {
            title: "Rework round 1".into(),
            scope: "s".into(),
            approach: "a".into(),
            notes: String::new(),
            tasks: tasks.into_iter().map(String::from).collect(),
        };
        assert_eq!(
            p.apply(add_phase(vec![])).unwrap_err(),
            "phase 3: has no tasks"
        );
        assert_eq!(p.apply(add_phase(vec!["E", "F"])).unwrap(), "3");
        assert_eq!(p.phases[2].tasks[1].id, "3.2");
        assert_eq!(Plan::parse(&serde_json::to_string(&p).unwrap()).unwrap(), p);
    }

    #[test]
    fn deviations_and_summaries_append() {
        let mut p = plan();
        let dev = |content: &str| PlanPatch::AppendDeviations {
            phase: "2".into(),
            content: content.into(),
        };
        p.apply(dev("first")).unwrap();
        p.apply(dev("second")).unwrap();
        assert_eq!(p.phases[1].deviations, "first\n\nsecond");
        assert_eq!(
            p.apply(dev(" ")).unwrap_err(),
            "phase 2: deviations are blank"
        );

        let summary = |kind, round| {
            PlanPatch::AddSummary(PlanSummary {
                kind,
                round,
                content: "shipped".into(),
            })
        };
        assert_eq!(p.apply(summary(PlanSummaryKind::Done, None)).unwrap(), "1");
        assert_eq!(
            p.apply(summary(PlanSummaryKind::Done, Some(1)))
                .unwrap_err(),
            "done summary takes no round"
        );
        assert_eq!(
            p.apply(summary(PlanSummaryKind::Rework, Some(0)))
                .unwrap_err(),
            "rework summary needs a round >= 1"
        );
        assert_eq!(
            p.apply(summary(PlanSummaryKind::Rework, Some(1))).unwrap(),
            "2"
        );
    }

    #[test]
    fn progress_tracks_the_current_task() {
        let mut p = plan();
        let start = p.progress();
        assert_eq!(
            start,
            PlanProgress {
                tasks_total: 3,
                tasks_done: 0,
                tasks_skipped: 0,
                current_phase_id: Some("1".into()),
                current_phase_title: Some("One".into()),
                current_task_id: Some("1.1".into()),
                current_task_title: Some("A".into()),
            }
        );

        set(&mut p, "2.1", PlanTaskStatus::InProgress).unwrap();
        let mid = p.progress();
        assert_eq!(mid.current_task_id.as_deref(), Some("2.1"));
        assert_eq!(mid.current_phase_title.as_deref(), Some("Two"));

        set(&mut p, "2.1", PlanTaskStatus::Done).unwrap();
        set(&mut p, "1.1", PlanTaskStatus::InProgress).unwrap();
        set(&mut p, "1.1", PlanTaskStatus::Done).unwrap();
        assert_eq!(p.progress().current_task_id.as_deref(), Some("1.2"));
        p.apply(PlanPatch::SetTaskStatus {
            task: "1.2".into(),
            status: PlanTaskStatus::Skipped,
            note: Some("n/a".into()),
        })
        .unwrap();
        let end = p.progress();
        assert_eq!((end.tasks_done, end.tasks_skipped), (2, 1));
        assert_eq!(end.current_phase_id, None);
        assert_eq!(end.current_task_id, None);
    }

    #[test]
    fn patch_proto_round_trip() {
        let patches = [
            PlanPatch::SetTaskStatus {
                task: "1.1".into(),
                status: PlanTaskStatus::Skipped,
                note: Some("n".into()),
            },
            PlanPatch::AddPhase {
                title: "t".into(),
                scope: "s".into(),
                approach: "a".into(),
                notes: String::new(),
                tasks: vec!["x".into()],
            },
            PlanPatch::AddSummary(PlanSummary {
                kind: PlanSummaryKind::Rework,
                round: Some(2),
                content: "c".into(),
            }),
        ];
        for p in patches {
            assert_eq!(PlanPatch::try_from(Op::from(p.clone())).unwrap(), p);
        }
        let bad = Op::SetTaskStatus(pb::PlanSetTaskStatus {
            task: "1.1".into(),
            status: "finished".into(),
            note: None,
        });
        assert!(PlanPatch::try_from(bad)
            .unwrap_err()
            .contains("unknown variant `finished`"));
    }

    #[test]
    fn markdown_renders_sections_and_checklist() {
        let mut p = plan();
        set(&mut p, "1.1", PlanTaskStatus::InProgress).unwrap();
        p.apply(PlanPatch::SetTaskStatus {
            task: "1.2".into(),
            status: PlanTaskStatus::Skipped,
            note: Some("n/a".into()),
        })
        .unwrap();
        p.apply(PlanPatch::AddSummary(PlanSummary {
            kind: PlanSummaryKind::Rework,
            round: Some(1),
            content: "fixed".into(),
        }))
        .unwrap();
        let md = p.to_markdown();
        for line in [
            "# MOD-1 Thing",
            "## Projects touched\n- project: Modula\n  - `x.rs`: new",
            "## Phase 1: One",
            "- [~] 1.1 A",
            "- [-] 1.2 B (skipped: n/a)",
            "### Notes\nn",
            "- [ ] 2.1 C",
            "## Risks & tradeoffs\nr",
            "## Rework round 1\nfixed",
        ] {
            assert!(md.contains(line), "missing {line:?} in\n{md}");
        }
    }
}

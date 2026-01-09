use crate::history_cell::HistoryCell;
use codex_council::CouncilEvent;
use codex_council::CouncilMode;
use codex_council::JobOutcome;
use ratatui::prelude::*;
use ratatui::style::Stylize;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Debug)]
pub struct CouncilProgressState {
    pub phases: Vec<PhaseState>,
    pub outcome: Option<JobOutcome>,
    pub summary: Option<String>,
    pub dry_run_failed: bool,
}

#[derive(Debug)]
pub struct CouncilProgressCell {
    pub job_id: String,
    pub mode: CouncilMode,
    pub target: PathBuf,
    pub head_sha: String,
    pub repo_dirty: bool,

    pub state: Arc<Mutex<CouncilProgressState>>,
}

#[derive(Debug)]
pub struct PhaseState {
    name: String,
    status: PhaseStatus,
    detail: String,
    notes: Vec<String>,
}

#[derive(Debug, PartialEq)]
pub enum PhaseStatus {
    Pending,
    Running,
    Done,
    Failed,
}

impl CouncilProgressCell {
    pub fn new(
        job_id: String,
        mode: CouncilMode,
        target: PathBuf,
        head_sha: String,
        repo_dirty: bool,
    ) -> Self {
        // Initialize standard phases based on mode
        let mut phases = Vec::new();
        phases.push(PhaseState::new("Isolation"));
        phases.push(PhaseState::new("Context"));
        if mode == CouncilMode::Fix {
            phases.push(PhaseState::new("Verify (Base)"));
        }
        phases.push(PhaseState::new("Criticism"));
        if mode == CouncilMode::Fix {
            phases.push(PhaseState::new("Planning"));
            phases.push(PhaseState::new("Implementation"));
            phases.push(PhaseState::new("Verification"));
        }

        let state = CouncilProgressState {
            phases,
            outcome: None,
            summary: None,
            dry_run_failed: false,
        };

        Self {
            job_id,
            mode,
            target,
            head_sha,
            repo_dirty,
            state: Arc::new(Mutex::new(state)),
        }
    }

    pub fn handle_event(&self, event: CouncilEvent) {
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        match event {
            CouncilEvent::PhaseStarted { phase, detail, .. } => {
                if let Some(p) = state.phases.iter_mut().find(|p| p.name == phase) {
                    p.status = PhaseStatus::Running;
                    p.detail = detail;
                } else {
                    let mut p = PhaseState::new(&phase);
                    p.status = PhaseStatus::Running;
                    p.detail = detail;
                    state.phases.push(p);
                }

                if let Some(idx) = state.phases.iter().position(|p| p.name == phase) {
                    for p in state.phases.iter_mut().take(idx) {
                        if p.status == PhaseStatus::Running || p.status == PhaseStatus::Pending {
                            p.status = PhaseStatus::Done;
                        }
                    }
                }
            }
            CouncilEvent::PhaseNote { phase, message } => {
                if let Some(p) = state.phases.iter_mut().find(|p| p.name == phase) {
                    p.notes.push(message);
                }
            }
            CouncilEvent::JobFinished {
                outcome,
                summary_line,
            } => {
                state.outcome = Some(outcome.clone());
                state.summary = Some(summary_line);
                for p in state.phases.iter_mut() {
                    if p.status == PhaseStatus::Running {
                        p.status = if outcome == JobOutcome::Success {
                            PhaseStatus::Done
                        } else {
                            PhaseStatus::Failed
                        };
                    }
                }
            }
            CouncilEvent::Error { phase, message } => {
                if let Some(p) = state.phases.iter_mut().find(|p| p.name == phase) {
                    p.status = PhaseStatus::Failed;
                    p.detail = message;
                }
            }
            _ => {}
        }
    }
}

impl PhaseState {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            status: PhaseStatus::Pending,
            detail: String::new(),
            notes: Vec::new(),
        }
    }
}

impl HistoryCell for CouncilProgressCell {
    fn display_lines(&self, _width: u16) -> Vec<Line<'static>> {
        let state = match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        let mut lines = Vec::new();

        // Header
        lines.push(Line::from(vec![
            "🧵 ThinThread ".red().bold(), // CHANGED TO RED
            format!("{:?} ", self.mode).dim(),
            self.target.to_string_lossy().to_string().into(),
            format!(" [{}]", self.job_id).dim(),
        ]));

        if self.repo_dirty {
            lines.push(Line::from(
                "⚠ Running on HEAD (uncommitted changes ignored)".magenta(),
            ));
        }

        // Phases
        for phase in &state.phases {
            let symbol = match phase.status {
                PhaseStatus::Pending => "  ".dim(),
                PhaseStatus::Running => "⟳ ".cyan(),
                PhaseStatus::Done => "✓ ".green(),
                PhaseStatus::Failed => "✗ ".red(),
            };

            let mut spans = vec![symbol];
            spans.push(phase.name.clone().into());
            if !phase.detail.is_empty()
                && (phase.status == PhaseStatus::Running || phase.status == PhaseStatus::Failed)
            {
                spans.push(": ".into());
                spans.push(phase.detail.clone().dim());
            }
            lines.push(Line::from(spans));

            for note in phase.notes.iter().rev().take(1) {
                lines.push(Line::from(vec!["    • ".dim(), note.clone().dim()]));
            }
        }

        // Footer / Outcome
        if let Some(outcome) = &state.outcome {
            let (color, text) = match outcome {
                JobOutcome::Success => (Color::Green, "Job Complete"),
                JobOutcome::Failure => (Color::Red, "Job Failed"),
                JobOutcome::Cancelled => (Color::Yellow, "Cancelled"),
            };
            lines.push(Line::from(vec![
                text.fg(color).bold(),
                ": ".into(),
                state.summary.clone().unwrap_or_default().into(),
            ]));

            if self.mode == CouncilMode::Fix && *outcome == JobOutcome::Success {
                lines.push(Line::from("")); // Spacer
                lines.push(Line::from(vec![
                    "  👉 NEXT STEP: ".yellow().bold(),
                    "Run ".white(),
                    format!("/thinthread apply {}", self.job_id).cyan().bold().underlined(),
                    " to update your files.".white(),
                ]));
                lines.push(Line::from("")); // Spacer
            } else if self.mode == CouncilMode::Review && *outcome == JobOutcome::Success {
                lines.push(Line::from("")); // Spacer
                lines.push(Line::from(vec![
                    "  👉 NEXT STEP: ".blue().bold(),
                    "To fix these issues, run ".white(),
                    format!("/thinthread fix {}", self.target.to_string_lossy()).cyan().bold(),
                    format!(" (Review ID: {})", self.job_id).dim(),
                ]));
                lines.push(Line::from("")); // Spacer
            }
        }

        crate::history_cell::with_border(lines)
    }
}

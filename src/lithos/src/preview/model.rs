//! Pure view-model types for the deploy plan preview.
//!
//! These types are deliberately decoupled from rendering and from the raw
//! resource-graph diff so that deploy logic, diff logic, and tests can build
//! and reason about a plan without touching a terminal.

use std::collections::BTreeMap;

use rbx_lithos::{
    diagnostics::DiagnosticReport,
    resource_graph::{Resource, ResourceGraph, ResourceGraphDiff},
    roblox_resource_manager::{RobloxInputs, RobloxOutputs, RobloxResource},
    state::{ReconciliationReport, VerificationStatus},
};

use super::summarize::{human_label, resource_type_label, risk_for_delete, summarize_change};

/// What will happen to a resource when deploy applies the plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionKind {
    /// The resource does not exist yet and will be created.
    Create,
    /// The resource exists and its inputs have changed.
    Update,
    /// The resource exists in state but is no longer in the desired graph and
    /// will be deleted.
    Delete,
    /// A dependency of this resource changed; the resource itself may be
    /// re-evaluated as a result.
    DependencyChange,
    /// The resource was found missing on Roblox during live reconciliation
    /// and will be re-created.
    DriftRecreate,
    /// Live reconciliation could not verify the resource; deploy will
    /// preserve persisted state and proceed.
    DriftUnknown,
}

impl ActionKind {
    /// Single-character marker that is meaningful without color (the prompt
    /// explicitly requires that color is not the only signal).
    pub fn marker(self) -> &'static str {
        match self {
            ActionKind::Create => "+",
            ActionKind::Update => "~",
            ActionKind::Delete => "-",
            ActionKind::DependencyChange => "○",
            ActionKind::DriftRecreate => "!",
            ActionKind::DriftUnknown => "?",
        }
    }

    pub fn verb(self) -> &'static str {
        match self {
            ActionKind::Create => "Create",
            ActionKind::Update => "Update",
            ActionKind::Delete => "Delete",
            ActionKind::DependencyChange => "Dependency change",
            ActionKind::DriftRecreate => "Drifted (will recreate)",
            ActionKind::DriftUnknown => "Drifted (verification inconclusive)",
        }
    }
}

/// How dangerous applying a row is. Used by the renderer to flag warnings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskLevel {
    Safe,
    Caution,
    Destructive,
}

impl RiskLevel {
    pub fn label(self) -> Option<&'static str> {
        match self {
            RiskLevel::Safe => None,
            RiskLevel::Caution => Some("caution"),
            RiskLevel::Destructive => Some("destructive"),
        }
    }
}

/// A single row in the preview.
#[derive(Debug, Clone)]
pub struct PlanRow {
    pub action: ActionKind,
    /// Short human label for the resource type, e.g. "Badge" or "Developer Product".
    pub resource_type: String,
    /// Stable graph id, kept for diagnostics.
    pub resource_id: String,
    /// Human-friendly name when one is available, falling back to the id.
    pub label: String,
    /// Field-level summary lines. Empty for additions/deletions where the
    /// header already says everything that matters.
    pub summary: Vec<String>,
    pub risk: RiskLevel,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlanCounts {
    pub creates: usize,
    pub updates: usize,
    pub deletes: usize,
    pub dependency_changes: usize,
    pub drift_recreate: usize,
    pub drift_unknown: usize,
}

impl PlanCounts {
    pub fn total_changes(&self) -> usize {
        // Drift-unknown isn't a change deploy will apply on its own, so we
        // exclude it from "is there anything to do" totals while still
        // reporting it for visibility.
        self.creates + self.updates + self.deletes + self.dependency_changes + self.drift_recreate
    }

    #[allow(dead_code)]
    pub fn has_destructive(&self) -> bool {
        self.deletes > 0
    }
}

#[derive(Debug, Clone, Default)]
pub struct Plan {
    pub rows: Vec<PlanRow>,
    pub counts: PlanCounts,
    pub preflight: DiagnosticReport,
    pub rollback: Option<RollbackSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollbackSummary {
    pub ready: bool,
    pub summary: String,
    pub details: Vec<String>,
}

impl Plan {
    /// Build a plan from the same diff and reconciliation data deploy will
    /// actually act on. Rows are returned in a stable display order:
    /// creates, updates, deletes, dependency changes, drift.
    pub fn build(
        diff: &ResourceGraphDiff,
        previous_graph: &ResourceGraph<RobloxResource, RobloxInputs, RobloxOutputs>,
        next_graph: &ResourceGraph<RobloxResource, RobloxInputs, RobloxOutputs>,
        reconciliation: Option<&ReconciliationReport>,
        preflight: Option<&DiagnosticReport>,
        rollback: Option<RollbackSummary>,
    ) -> Self {
        let prev_inputs = collect_inputs(previous_graph);
        let next_inputs = collect_inputs(next_graph);

        let mut rows: Vec<PlanRow> = Vec::new();
        let mut counts = PlanCounts::default();

        for (id, _addition) in diff.additions.iter() {
            if let Some(inputs) = next_inputs.get(id) {
                rows.push(PlanRow {
                    action: ActionKind::Create,
                    resource_type: resource_type_label(inputs).to_owned(),
                    resource_id: id.clone(),
                    label: human_label(inputs, id),
                    summary: Vec::new(),
                    risk: RiskLevel::Safe,
                });
                counts.creates += 1;
            }
        }

        for (id, _change) in diff.changes.iter() {
            if let (Some(prev), Some(next)) = (prev_inputs.get(id), next_inputs.get(id)) {
                rows.push(PlanRow {
                    action: ActionKind::Update,
                    resource_type: resource_type_label(next).to_owned(),
                    resource_id: id.clone(),
                    label: human_label(next, id),
                    summary: summarize_change(prev, next),
                    // Updates default to safe; specific summarizers can flag
                    // caution by returning lines, but we don't elevate risk
                    // automatically here to keep the model predictable.
                    risk: RiskLevel::Safe,
                });
                counts.updates += 1;
            }
        }

        for (id, _removal) in diff.removals.iter() {
            if let Some(inputs) = prev_inputs.get(id) {
                rows.push(PlanRow {
                    action: ActionKind::Delete,
                    resource_type: resource_type_label(inputs).to_owned(),
                    resource_id: id.clone(),
                    label: human_label(inputs, id),
                    summary: Vec::new(),
                    risk: risk_for_delete(inputs),
                });
                counts.deletes += 1;
            }
        }

        for (id, dep_change) in diff.dependency_changes.iter() {
            if let Some(inputs) = next_inputs.get(id) {
                let mut summary: Vec<String> = dep_change
                    .changed_dependencies
                    .iter()
                    .map(|d| format!("dependency changed: {}", d))
                    .collect();
                if summary.is_empty() {
                    summary.push("dependency changed".to_owned());
                }
                rows.push(PlanRow {
                    action: ActionKind::DependencyChange,
                    resource_type: resource_type_label(inputs).to_owned(),
                    resource_id: id.clone(),
                    label: human_label(inputs, id),
                    summary,
                    risk: RiskLevel::Safe,
                });
                counts.dependency_changes += 1;
            }
        }

        if let Some(report) = reconciliation {
            for (id, status) in report.entries.iter() {
                match status {
                    VerificationStatus::Missing => {
                        // Prefer the previous-graph inputs for naming; if the
                        // resource was already in the diff as an addition (it
                        // shouldn't be), avoid duplicating it.
                        if rows
                            .iter()
                            .any(|r| r.resource_id == *id && r.action == ActionKind::DriftRecreate)
                        {
                            continue;
                        }
                        let inputs = prev_inputs.get(id).or_else(|| next_inputs.get(id));
                        let (rtype, label) = match inputs {
                            Some(i) => (resource_type_label(i).to_owned(), human_label(i, id)),
                            None => ("Resource".to_owned(), id.clone()),
                        };
                        rows.push(PlanRow {
                            action: ActionKind::DriftRecreate,
                            resource_type: rtype,
                            resource_id: id.clone(),
                            label,
                            summary: vec![
                                "missing on Roblox; will be re-created on apply".to_owned()
                            ],
                            risk: RiskLevel::Caution,
                        });
                        counts.drift_recreate += 1;
                    }
                    VerificationStatus::Unknown(reason) => {
                        let inputs = prev_inputs.get(id).or_else(|| next_inputs.get(id));
                        let (rtype, label) = match inputs {
                            Some(i) => (resource_type_label(i).to_owned(), human_label(i, id)),
                            None => ("Resource".to_owned(), id.clone()),
                        };
                        rows.push(PlanRow {
                            action: ActionKind::DriftUnknown,
                            resource_type: rtype,
                            resource_id: id.clone(),
                            label,
                            summary: vec![format!("verification inconclusive: {}", reason)],
                            risk: RiskLevel::Caution,
                        });
                        counts.drift_unknown += 1;
                    }
                    _ => {}
                }
            }
        }

        Plan {
            rows,
            counts,
            preflight: preflight.cloned().unwrap_or_default(),
            rollback,
        }
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.counts.total_changes() == 0 && self.counts.drift_unknown == 0
    }
}

fn collect_inputs(
    graph: &ResourceGraph<RobloxResource, RobloxInputs, RobloxOutputs>,
) -> BTreeMap<String, RobloxInputs> {
    graph
        .get_resource_list()
        .into_iter()
        .map(|r| (r.get_id(), r.get_inputs()))
        .collect()
}

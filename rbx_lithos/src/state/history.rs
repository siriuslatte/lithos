use crate::{
    diagnostics::DeploymentDiagnostic,
    resource_graph::{EvaluateError, ResourceGraph},
    roblox_resource_manager::{RobloxInputs, RobloxOutputs, RobloxResource},
};

use super::v7::{
    build_applied_journal, DeploymentJournalEntry, DeploymentJournalStatus, ResourceStateV7,
};

pub fn build_failure_journal(
    baseline_graph: &ResourceGraph<RobloxResource, RobloxInputs, RobloxOutputs>,
    resulting_graph: &ResourceGraph<RobloxResource, RobloxInputs, RobloxOutputs>,
    error: &EvaluateError,
) -> Vec<DeploymentJournalEntry> {
    let mut journal = build_applied_journal(baseline_graph, resulting_graph);
    journal.extend(error.failures.iter().map(|failure| {
        DeploymentJournalEntry {
            resource_id: failure.resource_id.clone(),
            action: failure
                .error
                .diagnostics()
                .first()
                .and_then(|diagnostic| diagnostic.operation.as_ref())
                .map(|operation| operation.action)
                .unwrap_or(crate::diagnostics::OperationAction::Update),
            status: DeploymentJournalStatus::Failed,
            summary: failure.error.summary().to_owned(),
            diagnostics: failure.error.diagnostics().to_vec(),
        }
    }));
    journal
}

pub fn build_success_journal(
    baseline_graph: &ResourceGraph<RobloxResource, RobloxInputs, RobloxOutputs>,
    resulting_graph: &ResourceGraph<RobloxResource, RobloxInputs, RobloxOutputs>,
) -> Vec<DeploymentJournalEntry> {
    build_applied_journal(baseline_graph, resulting_graph)
}

pub fn rollback_snapshot(state: &ResourceStateV7, label: &str) -> Option<Vec<RobloxResource>> {
    state
        .latest_rollback_record(label)
        .map(|record| record.baseline.clone())
}

pub fn latest_deployment_diagnostics(
    state: &ResourceStateV7,
    label: &str,
) -> Vec<DeploymentDiagnostic> {
    state
        .latest_rollback_record(label)
        .map(|record| {
            record
                .journal
                .iter()
                .flat_map(|entry| entry.diagnostics.clone())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::{
        diagnostics::{DeploymentDiagnostic, DiagnosticCategory, OperationAction, OperationError},
        resource_graph::{
            EvaluateError, EvaluateResults, Resource, ResourceFailure, ResourceGraph,
        },
        roblox_resource_manager::{
            outputs::ExperienceOutputs, ExperienceActivationInputs, ExperienceInputs, RobloxInputs,
            RobloxOutputs, RobloxResource,
        },
        state::v7::{DeploymentKind, DeploymentStatus, EnvironmentStateV7, ResourceStateV7},
    };

    use super::*;

    fn experience(asset_id: u64) -> RobloxResource {
        RobloxResource::existing(
            "experience_singleton",
            RobloxInputs::Experience(ExperienceInputs { group_id: None }),
            RobloxOutputs::Experience(ExperienceOutputs {
                asset_id,
                start_place_id: asset_id + 1,
            }),
            &[],
        )
    }

    fn activation(enabled: bool, experience: &RobloxResource) -> RobloxResource {
        RobloxResource::existing(
            "experienceActivation_singleton",
            RobloxInputs::ExperienceActivation(ExperienceActivationInputs { is_active: enabled }),
            RobloxOutputs::ExperienceActivation,
            &[experience],
        )
    }

    #[test]
    fn rollback_snapshot_uses_latest_record_baseline() {
        let baseline = vec![experience(100)];
        let desired = vec![experience(200)];
        let record = crate::state::v7::DeploymentRecord {
            id: "deploy-2".to_owned(),
            kind: DeploymentKind::Deploy,
            status: DeploymentStatus::Failed,
            started_at: "2026-01-01T00:00:00Z".to_owned(),
            finished_at: None,
            baseline: baseline.clone(),
            desired,
            resulting: Vec::new(),
            journal: Vec::new(),
            summary: None,
            source_revision: None,
        };
        let state = ResourceStateV7 {
            environments: BTreeMap::from([(
                "prod".to_owned(),
                EnvironmentStateV7 {
                    current: vec![experience(300)],
                    deployments: vec![record],
                },
            )]),
        };

        let snapshot = rollback_snapshot(&state, "prod").unwrap();
        assert_eq!(snapshot.len(), 1);
        assert!(matches!(
            snapshot[0].get_outputs(),
            Some(RobloxOutputs::Experience(_))
        ));
    }

    #[test]
    fn failure_journal_captures_applied_and_failed_entries() {
        let baseline_experience = experience(100);
        let baseline_activation = activation(false, &baseline_experience);
        let baseline_graph =
            ResourceGraph::new(&[baseline_experience.clone(), baseline_activation]);

        let resulting_experience = experience(100);
        let resulting_activation = activation(true, &resulting_experience);
        let mut resulting_graph = ResourceGraph::new(&[resulting_experience, resulting_activation]);

        let evaluate_error = EvaluateError {
            results: EvaluateResults {
                updated_count: 1,
                ..Default::default()
            },
            failures: vec![ResourceFailure {
                resource_id: "badge_icon".to_owned(),
                error: OperationError::from_diagnostic(
                    DeploymentDiagnostic::error(
                        "rollback-failed",
                        DiagnosticCategory::Rollback,
                        "Rollback failed for badge_icon.",
                    )
                    .with_operation(crate::diagnostics::OperationContext {
                        action: OperationAction::Update,
                        resource_id: Some("badge_icon".to_owned()),
                        resource_type: "BadgeIcon".to_owned(),
                        auth_model: "cookie-session".to_owned(),
                        creator_target: None,
                        endpoint: None,
                        file_path: None,
                    }),
                ),
            }],
        };

        let journal = build_failure_journal(&baseline_graph, &mut resulting_graph, &evaluate_error);

        assert!(journal.iter().any(|entry| {
            entry.resource_id == "experienceActivation_singleton"
                && entry.status == crate::state::v7::DeploymentJournalStatus::Applied
        }));
        assert!(journal.iter().any(|entry| {
            entry.resource_id == "badge_icon"
                && entry.status == crate::state::v7::DeploymentJournalStatus::Failed
        }));
    }
}

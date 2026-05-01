use std::collections::BTreeMap;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::{
    diagnostics::{DeploymentDiagnostic, OperationAction},
    resource_graph::ResourceGraph,
    roblox_resource_manager::{RobloxInputs, RobloxOutputs, RobloxResource},
};

use super::v6::ResourceStateV6;

const MAX_DEPLOYMENT_HISTORY: usize = 10;

#[derive(Serialize, Deserialize, Clone)]
pub struct ResourceStateV7 {
    pub environments: BTreeMap<String, EnvironmentStateV7>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentStateV7 {
    #[serde(default)]
    pub current: Vec<RobloxResource>,
    #[serde(default)]
    pub deployments: Vec<DeploymentRecord>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentRecord {
    pub id: String,
    pub kind: DeploymentKind,
    pub status: DeploymentStatus,
    pub started_at: String,
    pub finished_at: Option<String>,
    #[serde(default)]
    pub baseline: Vec<RobloxResource>,
    #[serde(default)]
    pub desired: Vec<RobloxResource>,
    #[serde(default)]
    pub resulting: Vec<RobloxResource>,
    #[serde(default)]
    pub journal: Vec<DeploymentJournalEntry>,
    pub summary: Option<String>,
    pub source_revision: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DeploymentKind {
    Deploy,
    Undo,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DeploymentStatus {
    InProgress,
    Succeeded,
    Failed,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentJournalEntry {
    pub resource_id: String,
    pub action: OperationAction,
    pub status: DeploymentJournalStatus,
    pub summary: String,
    #[serde(default)]
    pub diagnostics: Vec<DeploymentDiagnostic>,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DeploymentJournalStatus {
    Applied,
    Failed,
}

impl From<ResourceStateV6> for ResourceStateV7 {
    fn from(state: ResourceStateV6) -> Self {
        Self {
            environments: state
                .environments
                .into_iter()
                .map(|(label, resources)| {
                    (
                        label,
                        EnvironmentStateV7 {
                            current: resources,
                            deployments: Vec::new(),
                        },
                    )
                })
                .collect(),
        }
    }
}

impl ResourceStateV7 {
    pub fn ensure_environment_mut(&mut self, label: &str) -> &mut EnvironmentStateV7 {
        self.environments.entry(label.to_owned()).or_default()
    }

    pub fn environment(&self, label: &str) -> Option<&EnvironmentStateV7> {
        self.environments.get(label)
    }

    pub fn current_resources(&self, label: &str) -> Option<&Vec<RobloxResource>> {
        self.environment(label)
            .map(|environment| &environment.current)
    }

    pub fn begin_deployment(
        &mut self,
        label: &str,
        kind: DeploymentKind,
        baseline: Vec<RobloxResource>,
        desired: Vec<RobloxResource>,
        source_revision: Option<String>,
    ) -> String {
        let id = format!(
            "{}-{}",
            match kind {
                DeploymentKind::Deploy => "deploy",
                DeploymentKind::Undo => "undo",
            },
            Utc::now().timestamp_millis()
        );

        let environment = self.ensure_environment_mut(label);
        environment.deployments.push(DeploymentRecord {
            id: id.clone(),
            kind,
            status: DeploymentStatus::InProgress,
            started_at: Utc::now().to_rfc3339(),
            finished_at: None,
            baseline,
            desired,
            resulting: Vec::new(),
            journal: Vec::new(),
            summary: None,
            source_revision,
        });
        trim_history(environment);

        id
    }

    pub fn complete_deployment(
        &mut self,
        label: &str,
        deployment_id: &str,
        status: DeploymentStatus,
        resulting: Vec<RobloxResource>,
        journal: Vec<DeploymentJournalEntry>,
        summary: Option<String>,
    ) {
        let environment = self.ensure_environment_mut(label);
        if let Some(record) = environment
            .deployments
            .iter_mut()
            .rev()
            .find(|record| record.id == deployment_id)
        {
            record.status = status;
            record.finished_at = Some(Utc::now().to_rfc3339());
            record.resulting = resulting.clone();
            record.journal = journal;
            record.summary = summary;
        }

        if matches!(status, DeploymentStatus::Succeeded) {
            environment.current = resulting;
        }
    }

    pub fn update_deployment_progress(
        &mut self,
        label: &str,
        deployment_id: &str,
        resulting: Vec<RobloxResource>,
        journal: Vec<DeploymentJournalEntry>,
        summary: Option<String>,
    ) {
        let environment = self.ensure_environment_mut(label);
        if let Some(record) = environment
            .deployments
            .iter_mut()
            .rev()
            .find(|record| record.id == deployment_id)
        {
            record.status = DeploymentStatus::InProgress;
            record.resulting = resulting;
            record.journal = journal;
            record.summary = summary;
        }
    }

    pub fn latest_rollback_record(&self, label: &str) -> Option<&DeploymentRecord> {
        self.environment(label)?
            .deployments
            .iter()
            .rev()
            .find(|record| {
                !record.baseline.is_empty()
                    && matches!(
                        record.status,
                        DeploymentStatus::InProgress
                            | DeploymentStatus::Failed
                            | DeploymentStatus::Succeeded
                    )
            })
    }
}

pub fn build_applied_journal(
    baseline_graph: &ResourceGraph<RobloxResource, RobloxInputs, RobloxOutputs>,
    resulting_graph: &ResourceGraph<RobloxResource, RobloxInputs, RobloxOutputs>,
) -> Vec<DeploymentJournalEntry> {
    let mut journal = Vec::new();

    if let Ok(diff) = resulting_graph.diff(baseline_graph) {
        journal.extend(
            diff.additions
                .keys()
                .map(|resource_id| DeploymentJournalEntry {
                    resource_id: resource_id.clone(),
                    action: OperationAction::Create,
                    status: DeploymentJournalStatus::Applied,
                    summary: format!("Applied create for {}", resource_id),
                    diagnostics: Vec::new(),
                }),
        );
        journal.extend(
            diff.changes
                .keys()
                .map(|resource_id| DeploymentJournalEntry {
                    resource_id: resource_id.clone(),
                    action: OperationAction::Update,
                    status: DeploymentJournalStatus::Applied,
                    summary: format!("Applied update for {}", resource_id),
                    diagnostics: Vec::new(),
                }),
        );
        journal.extend(
            diff.removals
                .keys()
                .map(|resource_id| DeploymentJournalEntry {
                    resource_id: resource_id.clone(),
                    action: OperationAction::Delete,
                    status: DeploymentJournalStatus::Applied,
                    summary: format!("Applied delete for {}", resource_id),
                    diagnostics: Vec::new(),
                }),
        );
    }

    journal
}

fn trim_history(environment: &mut EnvironmentStateV7) {
    if environment.deployments.len() > MAX_DEPLOYMENT_HISTORY {
        let overflow = environment.deployments.len() - MAX_DEPLOYMENT_HISTORY;
        environment.deployments.drain(0..overflow);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        resource_graph::Resource,
        roblox_resource_manager::{ExperienceInputs, RobloxInputs},
    };

    fn resource(id: &str) -> RobloxResource {
        RobloxResource::new(
            id,
            RobloxInputs::Experience(ExperienceInputs { group_id: None }),
            &[],
        )
    }

    #[test]
    fn migrates_v6_into_environment_state() {
        let migrated = ResourceStateV7::from(ResourceStateV6 {
            environments: BTreeMap::from([(
                "prod".to_owned(),
                vec![resource("experience_singleton")],
            )]),
        });

        assert_eq!(migrated.current_resources("prod").unwrap().len(), 1);
        assert!(migrated.environment("prod").unwrap().deployments.is_empty());
    }

    #[test]
    fn prefers_latest_rollback_record() {
        let mut state = ResourceStateV7 {
            environments: BTreeMap::new(),
        };
        let baseline = vec![resource("baseline")];
        let desired = vec![resource("desired")];
        let first = state.begin_deployment(
            "prod",
            DeploymentKind::Deploy,
            baseline.clone(),
            desired.clone(),
            None,
        );
        state.complete_deployment(
            "prod",
            &first,
            DeploymentStatus::Succeeded,
            desired.clone(),
            Vec::new(),
            Some("ok".to_owned()),
        );
        let second = state.begin_deployment(
            "prod",
            DeploymentKind::Deploy,
            desired,
            baseline.clone(),
            None,
        );

        assert_eq!(state.latest_rollback_record("prod").unwrap().id, second);

        state.complete_deployment(
            "prod",
            &second,
            DeploymentStatus::Failed,
            baseline,
            Vec::new(),
            Some("failed".to_owned()),
        );

        assert_eq!(state.latest_rollback_record("prod").unwrap().id, second);
    }

    #[test]
    fn updates_in_progress_deployment_without_advancing_current() {
        let mut state = ResourceStateV7 {
            environments: BTreeMap::from([(
                "prod".to_owned(),
                EnvironmentStateV7 {
                    current: vec![resource("current")],
                    deployments: Vec::new(),
                },
            )]),
        };

        let deployment_id = state.begin_deployment(
            "prod",
            DeploymentKind::Deploy,
            vec![resource("baseline")],
            vec![resource("desired")],
            None,
        );
        let partial_result = vec![resource("partial")];
        let journal = vec![DeploymentJournalEntry {
            resource_id: "partial".to_owned(),
            action: OperationAction::Create,
            status: DeploymentJournalStatus::Applied,
            summary: "Applied create for partial".to_owned(),
            diagnostics: Vec::new(),
        }];

        state.update_deployment_progress(
            "prod",
            &deployment_id,
            partial_result.clone(),
            journal.clone(),
            Some("In progress".to_owned()),
        );

        let environment = state.environment("prod").unwrap();
        assert_eq!(environment.current.len(), 1);
        assert_eq!(environment.current[0].get_id(), "current");

        let record = environment
            .deployments
            .iter()
            .find(|record| record.id == deployment_id)
            .unwrap();
        assert!(matches!(record.status, DeploymentStatus::InProgress));
        assert_eq!(record.resulting.len(), partial_result.len());
        assert_eq!(record.journal.len(), journal.len());
        assert_eq!(record.summary.as_deref(), Some("In progress"));
    }
}

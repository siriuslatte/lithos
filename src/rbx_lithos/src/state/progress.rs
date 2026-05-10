use std::path::Path;

use async_trait::async_trait;

use crate::{
    config::StateConfig,
    resource_graph::{
        EvaluateError, EvaluateProgressHandler, EvaluateResults, ResourceFailure, ResourceGraph,
    },
    roblox_resource_manager::{RobloxInputs, RobloxOutputs, RobloxResource},
};

use super::{
    history::{build_failure_journal, build_success_journal},
    io::{save_state, ResourceStateVLatest},
};

pub struct DeploymentProgressWriter<'a> {
    project_path: &'a Path,
    state_config: &'a StateConfig,
    state: &'a mut ResourceStateVLatest,
    environment_label: &'a str,
    deployment_id: &'a str,
    baseline_graph: &'a ResourceGraph<RobloxResource, RobloxInputs, RobloxOutputs>,
}

impl<'a> DeploymentProgressWriter<'a> {
    pub fn new(
        project_path: &'a Path,
        state_config: &'a StateConfig,
        state: &'a mut ResourceStateVLatest,
        environment_label: &'a str,
        deployment_id: &'a str,
        baseline_graph: &'a ResourceGraph<RobloxResource, RobloxInputs, RobloxOutputs>,
    ) -> Self {
        Self {
            project_path,
            state_config,
            state,
            environment_label,
            deployment_id,
            baseline_graph,
        }
    }

    fn progress_summary(results: &EvaluateResults, failures: &[ResourceFailure]) -> String {
        if failures.is_empty() {
            format!(
                "In progress: applied {} create(s), {} update(s), {} delete(s), {} noop(s), {} skip(s)",
                results.created_count,
                results.updated_count,
                results.deleted_count,
                results.noop_count,
                results.skipped_count
            )
        } else {
            format!(
                "In progress: applied {} create(s), {} update(s), {} delete(s), {} noop(s), {} skip(s), {} failure(s)",
                results.created_count,
                results.updated_count,
                results.deleted_count,
                results.noop_count,
                results.skipped_count,
                failures.len()
            )
        }
    }
}

#[async_trait(?Send)]
impl EvaluateProgressHandler<RobloxResource, RobloxInputs, RobloxOutputs>
    for DeploymentProgressWriter<'_>
{
    async fn persist_progress(
        &mut self,
        current_graph: &ResourceGraph<RobloxResource, RobloxInputs, RobloxOutputs>,
        results: &EvaluateResults,
        failures: &[ResourceFailure],
    ) -> Result<(), String> {
        let journal = if failures.is_empty() {
            build_success_journal(self.baseline_graph, current_graph)
        } else {
            build_failure_journal(
                self.baseline_graph,
                current_graph,
                &EvaluateError {
                    results: results.clone(),
                    failures: failures.to_vec(),
                },
            )
        };

        self.state.update_deployment_progress(
            self.environment_label,
            self.deployment_id,
            current_graph.get_resource_list(),
            journal,
            Some(Self::progress_summary(results, failures)),
        );

        save_state(self.project_path, self.state_config, self.state).await
    }
}

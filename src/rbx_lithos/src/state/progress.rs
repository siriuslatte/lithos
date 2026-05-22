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
    io::{save_state_cas, ResourceStateVLatest, SaveTarget},
    lock::{heartbeat_environment_lock, EnvironmentLockSession},
    store::StateHandle,
};

pub struct DeploymentProgressWriter<'a> {
    project_path: &'a Path,
    state_config: &'a StateConfig,
    state: &'a mut ResourceStateVLatest,
    state_handle: &'a mut StateHandle,
    environment_label: &'a str,
    deployment_id: &'a str,
    baseline_graph: &'a ResourceGraph<RobloxResource, RobloxInputs, RobloxOutputs>,
    lock: Option<&'a EnvironmentLockSession>,
}

impl<'a> DeploymentProgressWriter<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        project_path: &'a Path,
        state_config: &'a StateConfig,
        state: &'a mut ResourceStateVLatest,
        state_handle: &'a mut StateHandle,
        environment_label: &'a str,
        deployment_id: &'a str,
        baseline_graph: &'a ResourceGraph<RobloxResource, RobloxInputs, RobloxOutputs>,
        lock: Option<&'a EnvironmentLockSession>,
    ) -> Self {
        Self {
            project_path,
            state_config,
            state,
            state_handle,
            environment_label,
            deployment_id,
            baseline_graph,
            lock,
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

        if let Some(session) = self.lock {
            heartbeat_environment_lock(
                self.project_path,
                self.state_config,
                self.state,
                self.state_handle,
                session,
            )
            .await?;
        }

        let baseline = self.state.environment(self.environment_label).cloned();
        let target = SaveTarget::new(self.environment_label, baseline);
        let new_handle = save_state_cas(
            self.project_path,
            self.state_config,
            self.state,
            self.state_handle,
            &target,
        )
        .await?;
        *self.state_handle = new_handle;
        Ok(())
    }
}

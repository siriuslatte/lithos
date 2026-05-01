use yansi::Paint;

use rbx_lithos::{
    config::load_project_config,
    diagnostics::{run_preflight_checks, DiagnosticReport, PreflightAuthContext},
    project::{load_project, Project},
    resource_graph::{EvaluateResults, Resource, ResourceGraph},
    roblox_resource_manager::{RobloxOutputs, RobloxResource, RobloxResourceManager},
    state::{
        build_failure_journal, build_success_journal, import_graph, save_state,
        v7::{DeploymentKind, DeploymentStatus},
        DeploymentProgressWriter,
    },
};

use crate::preview::{
    model::{Plan, RollbackSummary},
    render::{preview_and_confirm, Decision, PreviewOptions},
};

fn log_preflight_report(report: &DiagnosticReport) {
    logger::start_action("Running rollback preflight checks:");

    if report.blocking.is_empty() && report.warnings.is_empty() {
        logger::end_action("Succeeded");
        return;
    }

    for diagnostic in &report.blocking {
        logger::log(format!("{} {}", Paint::red("!"), diagnostic.summary));
        if let Some(detail) = &diagnostic.detail {
            logger::log(format!("    {}", detail));
        }
        for next_step in &diagnostic.next_steps {
            logger::log(format!("    next: {}", next_step));
        }
    }

    for diagnostic in &report.warnings {
        logger::log(format!("{} {}", Paint::yellow("?"), diagnostic.summary));
        if let Some(detail) = &diagnostic.detail {
            logger::log(format!("    {}", detail));
        }
    }

    if report.has_blocking() {
        logger::end_action(Paint::red(format!(
            "Blocked with {} error(s) and {} warning(s)",
            report.blocking.len(),
            report.warnings.len()
        )));
    } else {
        logger::end_action(format!(
            "Succeeded with {} warning(s)",
            report.warnings.len()
        ));
    }
}

fn extract_experience_id(resources: &[RobloxResource]) -> Option<u64> {
    resources
        .iter()
        .find_map(|resource| match resource.get_outputs() {
            Some(RobloxOutputs::Experience(outputs)) => Some(outputs.asset_id),
            _ => None,
        })
}

fn rollback_plan_summary(record: &rbx_lithos::state::v7::DeploymentRecord) -> RollbackSummary {
    let details = vec![
        format!("Rolling back to the snapshot captured at {}.", record.started_at),
        format!(
            "The most recent deployment record is currently marked {}.",
            match record.status {
                DeploymentStatus::InProgress => "in progress",
                DeploymentStatus::Succeeded => "succeeded",
                DeploymentStatus::Failed => "failed",
            }
        ),
        "Undo is best-effort: Roblox deploys are not transactional, so Lithos will reconcile live state and then drive resources back toward this snapshot.".to_owned(),
    ];

    RollbackSummary {
        ready: true,
        summary: "Lithos will restore the last known good checkpoint for this environment."
            .to_owned(),
        details,
    }
}

pub async fn run(
    project: Option<&str>,
    environment: Option<&str>,
    allow_purchases: bool,
    preview_options: PreviewOptions,
) -> i32 {
    logger::start_action("Loading project:");
    let (project_path, config) = match load_project_config(project) {
        Ok(v) => v,
        Err(e) => {
            logger::end_action(Paint::red(e));
            return 1;
        }
    };
    let Project {
        current_graph,
        mut state,
        environment_config,
        target_config,
        payment_source,
        state_config,
        owner_config,
    } = match load_project(project_path.clone(), config, environment).await {
        Ok(Some(v)) => v,
        Ok(None) => {
            logger::end_action("No rollback target available");
            return 0;
        }
        Err(e) => {
            logger::end_action(Paint::red(e));
            return 1;
        }
    };

    let Some(rollback_record) = state
        .latest_rollback_record(&environment_config.label)
        .cloned()
    else {
        logger::end_action(Paint::red(
            "No rollback snapshot is recorded for this environment yet.",
        ));
        return 1;
    };
    let rollback_resources = rollback_record.baseline.clone();
    logger::end_action("Succeeded");

    logger::start_action("Loading live environment:");
    let mut resource_manager = match RobloxResourceManager::new(&project_path, payment_source).await
    {
        Ok(v) => v,
        Err(e) => {
            logger::end_action(Paint::red(e));
            return 1;
        }
    };

    let import_target_id = extract_experience_id(&rollback_record.resulting)
        .or_else(|| extract_experience_id(&rollback_resources))
        .or_else(|| match current_graph.get_outputs("experience_singleton") {
            Some(RobloxOutputs::Experience(outputs)) => Some(outputs.asset_id),
            _ => None,
        });

    let live_graph = match import_target_id {
        Some(target_id) => match import_graph(resource_manager.api(), target_id).await {
            Ok(graph) => {
                logger::end_action("Succeeded");
                graph
            }
            Err(e) => {
                logger::end_action(Paint::red(e));
                return 1;
            }
        },
        None => {
            logger::end_action("No live experience found; assuming an empty managed state");
            ResourceGraph::new(&Vec::new())
        }
    };

    let mut rollback_graph = ResourceGraph::new(&rollback_resources);
    let open_cloud_key = resource_manager
        .introspect_open_cloud_api_key()
        .await
        .ok()
        .flatten();
    let preflight_report = run_preflight_checks(
        project_path.as_path(),
        &target_config,
        &owner_config,
        &live_graph,
        &rollback_graph,
        &PreflightAuthContext {
            open_cloud_api_key_present: resource_manager.has_open_cloud_api_key(),
            open_cloud_key,
        },
    );
    log_preflight_report(&preflight_report);
    if preflight_report.has_blocking() {
        return 1;
    }

    let diff = match rollback_graph.diff(&live_graph) {
        Ok(diff) => diff,
        Err(e) => {
            logger::log(Paint::red(format!(
                "Failed to compute rollback plan diff: {}",
                e
            )));
            return 1;
        }
    };
    let plan = Plan::build(
        &diff,
        &live_graph,
        &rollback_graph,
        None,
        Some(&preflight_report),
        Some(rollback_plan_summary(&rollback_record)),
    );

    match preview_and_confirm(&plan, preview_options) {
        Decision::Approve => {}
        Decision::Cancel => {
            logger::log(Paint::yellow("Undo cancelled by user."));
            return 2;
        }
    }

    logger::start_action("Checkpointing pre-undo state:");
    let deployment_id = state.begin_deployment(
        &environment_config.label,
        DeploymentKind::Undo,
        current_graph.get_resource_list(),
        rollback_graph.get_resource_list(),
        None,
    );
    match save_state(&project_path, &state_config, &state).await {
        Ok(_) => logger::end_action("Succeeded"),
        Err(e) => {
            logger::end_action(Paint::red(e));
            return 1;
        }
    };

    logger::start_action("Undoing resources:");
    let results = {
        let mut progress_writer = DeploymentProgressWriter::new(
            project_path.as_path(),
            &state_config,
            &mut state,
            &environment_config.label,
            &deployment_id,
            &live_graph,
        );
        rollback_graph
            .evaluate_with_progress(
                &live_graph,
                &mut resource_manager,
                allow_purchases,
                Some(&mut progress_writer),
            )
            .await
    };
    match &results {
        Ok(results) => match results {
            EvaluateResults {
                created_count: 0,
                updated_count: 0,
                deleted_count: 0,
                skipped_count: 0,
                ..
            } => logger::end_action("No changes required"),
            EvaluateResults {
                created_count,
                updated_count,
                deleted_count,
                noop_count,
                skipped_count,
            } => logger::end_action(format!(
                "Succeeded with {} create(s), {} update(s), {} delete(s), {} noop(s), {} skip(s)",
                created_count, updated_count, deleted_count, noop_count, skipped_count
            )),
        },
        Err(error) => {
            logger::end_action(Paint::red(error));
            if error.applied_mutation_count() > 0 {
                logger::log(Paint::yellow(format!(
                    "Undo failed after {} applied mutation(s). The environment may still be partially updated.",
                    error.applied_mutation_count()
                )));
            }
            logger::log(format!(
                "Recovery: inspect the rollback journal in state and rerun {} after correcting the issue.",
                Paint::cyan(format!(
                    "lithos undo --environment {}",
                    environment_config.label
                ))
            ));
        }
    }

    let journal = match &results {
        Ok(_) => build_success_journal(&live_graph, &rollback_graph),
        Err(error) => build_failure_journal(&live_graph, &rollback_graph, error),
    };
    let summary = match &results {
        Ok(EvaluateResults {
            created_count,
            updated_count,
            deleted_count,
            noop_count,
            skipped_count,
        }) => Some(format!(
            "Undo applied {} create(s), {} update(s), {} delete(s), {} noop(s), {} skip(s)",
            created_count, updated_count, deleted_count, noop_count, skipped_count
        )),
        Err(error) => Some(format!(
            "Undo failed after {} applied mutation(s) and {} recorded failure(s)",
            error.applied_mutation_count(),
            error.failure_count()
        )),
    };

    state.complete_deployment(
        &environment_config.label,
        &deployment_id,
        if results.is_ok() {
            DeploymentStatus::Succeeded
        } else {
            DeploymentStatus::Failed
        },
        rollback_graph.get_resource_list(),
        journal,
        summary,
    );

    logger::start_action("Saving state:");
    match save_state(&project_path, &state_config, &state).await {
        Ok(_) => logger::end_action("Succeeded"),
        Err(e) => {
            logger::end_action(Paint::red(e));
            return 1;
        }
    };

    match results {
        Ok(_) => 0,
        Err(_) => 1,
    }
}

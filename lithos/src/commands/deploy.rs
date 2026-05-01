use std::{path::PathBuf, process::Command, str};

use yansi::Paint;

use rbx_lithos::{
    config::{load_project_config, TargetConfig},
    diagnostics::{run_preflight_checks, DiagnosticReport, PreflightAuthContext},
    project::{load_project, Project},
    resource_graph::{EvaluateResults, ResourceGraph},
    roblox_resource_manager::{RobloxInputs, RobloxOutputs, RobloxResource, RobloxResourceManager},
    state::v7::{DeploymentKind, DeploymentStatus},
    state::{
        build_failure_journal, build_success_journal, get_desired_graph, reconcile_graph,
        save_state, DeploymentProgressWriter, ReconciliationReport, RobloxLiveStateVerifier,
        VerificationStatus,
    },
};

use crate::preview::{
    model::{Plan, RollbackSummary},
    render::{preview_and_confirm, Decision, PreviewOptions},
};
use crate::tui::progress::Spinner;

fn run_command(dir: PathBuf, command: &str) -> std::io::Result<std::process::Output> {
    if cfg!(target_os = "windows") {
        Command::new("cmd")
            .current_dir(dir)
            .arg("/C")
            .arg(command)
            .output()
    } else {
        Command::new("sh")
            .current_dir(dir)
            .arg("-c")
            .arg(command)
            .output()
    }
}

fn tag_commit(
    project_path: PathBuf,
    target_config: &TargetConfig,
    next_graph: &ResourceGraph<RobloxResource, RobloxInputs, RobloxOutputs>,
    previous_graph: &ResourceGraph<RobloxResource, RobloxInputs, RobloxOutputs>,
) -> Result<u32, String> {
    let mut tag_count: u32 = 0;

    match target_config {
        TargetConfig::Experience(target_config) => {
            for label in target_config.places.as_ref().unwrap().keys() {
                let resource_id = format!("placeFile_{}", label);

                let previous_outputs = previous_graph.get_outputs(&resource_id);
                let next_outputs = next_graph.get_outputs(&resource_id);

                let tag_version = match (previous_outputs, next_outputs) {
                    (None, Some(RobloxOutputs::PlaceFile(next))) => Some(next.version),
                    (
                        Some(RobloxOutputs::PlaceFile(previous)),
                        Some(RobloxOutputs::PlaceFile(next)),
                    ) if next.version != previous.version => Some(next.version),
                    _ => None,
                };

                if let Some(version) = tag_version {
                    logger::log(format!(
                        "Place {} was updated to version {}",
                        Paint::cyan(label),
                        Paint::cyan(version)
                    ));
                    let tag = format!("{}-v{}", label, version);
                    logger::log(format!("Tagging commit with {}", Paint::cyan(tag.clone())));

                    tag_count += 1;
                    run_command(project_path.clone(), &format!("git tag {}", tag))
                        .map_err(|e| format!("Unable to tag the current commit\n\t{}", e))?;
                }
            }
        }
    }

    if tag_count > 0 {
        run_command(project_path, "git push --tags")
            .map_err(|e| format!("Unable to push tags to remote\n\t{}", e))?;
    }

    Ok(tag_count)
}

fn log_target_results(
    target_config: &TargetConfig,
    graph: &ResourceGraph<RobloxResource, RobloxInputs, RobloxOutputs>,
) {
    logger::start_action("Target results:");
    match target_config {
        TargetConfig::Experience(target_config) => {
            let experience_outputs = match graph.get_outputs("experience_singleton") {
                Some(RobloxOutputs::Experience(outputs)) => Some(outputs),
                _ => None,
            };
            logger::log("Experience:");
            if let Some(outputs) = experience_outputs {
                logger::log(format!(
                    "  https://www.roblox.com/games/{}",
                    outputs.start_place_id
                ));
            } else {
                logger::log(Paint::red("  no outputs"));
            }
            logger::log("");

            logger::log("Places:");
            for label in target_config.places.as_ref().unwrap().keys() {
                let resource_id = format!("place_{}", label);

                let place_outputs = match graph.get_outputs(&resource_id) {
                    Some(RobloxOutputs::Place(outputs)) => Some(outputs),
                    _ => None,
                };
                if let Some(outputs) = place_outputs {
                    logger::log(format!(
                        "  {}: https://www.roblox.com/games/{}",
                        label, outputs.asset_id
                    ));
                } else {
                    logger::log(format!("  {}: {}", label, Paint::red("no outputs")));
                }
            }
        }
    }
    logger::end_action_without_message();
}

fn log_preflight_report(report: &DiagnosticReport) {
    logger::start_action("Running preflight checks:");

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

fn rollback_readiness_summary(
    state_config: &rbx_lithos::config::StateConfig,
    current_graph: &ResourceGraph<RobloxResource, RobloxInputs, RobloxOutputs>,
) -> RollbackSummary {
    let backend = match state_config {
        rbx_lithos::config::StateConfig::Local => "local state file".to_owned(),
        rbx_lithos::config::StateConfig::LocalKey(key) => {
            format!("local keyed state file ({})", key)
        }
        rbx_lithos::config::StateConfig::Remote(config) => format!("remote state ({})", config),
    };

    let mut details = vec![format!(
        "Lithos will checkpoint the current environment in {} before apply starts.",
        backend
    )];
    if current_graph.get_resource_list().is_empty() {
        details.push(
            "This environment has no prior managed resources, so undo will restore the environment back to an empty managed state if this deploy fails or needs to be reverted.".to_owned(),
        );
    } else {
        details.push(
            "Undo will use the last known good snapshot plus live import to plan a best-effort rollback after failure or a bad deploy.".to_owned(),
        );
    }

    RollbackSummary {
        ready: true,
        summary: "A last-known-good checkpoint will be captured before apply.".to_owned(),
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
            logger::end_action("No deployment necessary");
            return 0;
        }
        Err(e) => {
            logger::end_action(Paint::red(e));
            return 1;
        }
    };
    let mut next_graph =
        match get_desired_graph(project_path.as_path(), &target_config, &owner_config) {
            Ok(v) => v,
            Err(e) => {
                logger::end_action(Paint::red(e));
                return 1;
            }
        };
    logger::end_action("Succeeded");

    logger::start_action("Deploying resources:");
    let mut resource_manager = match RobloxResourceManager::new(&project_path, payment_source).await
    {
        Ok(v) => v,
        Err(e) => {
            logger::end_action(Paint::red(e));
            return 1;
        }
    };

    // Live reconciliation: verify persisted state against the Roblox platform
    let open_cloud_key = resource_manager
        .introspect_open_cloud_api_key()
        .await
        .ok()
        .flatten();
    let preflight_report = run_preflight_checks(
        project_path.as_path(),
        &target_config,
        &owner_config,
        &current_graph,
        &next_graph,
        &PreflightAuthContext {
            open_cloud_api_key_present: resource_manager.has_open_cloud_api_key(),
            open_cloud_key,
        },
    );
    log_preflight_report(&preflight_report);
    if preflight_report.has_blocking() {
        logger::end_action(Paint::red("Apply aborted by preflight diagnostics"));
        return 1;
    }

    // before evaluating the graph. This catches out-of-band deletions and
    // ensures we do not attempt to update or delete assets that no longer
    // exist. Resources whose status is `Verified`, `Skipped`, or `Unknown`
    // are preserved unchanged; only confirmed-`Missing` resources are
    // dropped from the previous-state graph (so the next phase will
    // re-create them rather than fail on update).
    let (current_graph, reconciliation_report): (
        ResourceGraph<RobloxResource, RobloxInputs, RobloxOutputs>,
        ReconciliationReport,
    ) = {
        let spinner = Spinner::start("Reconciling state with Roblox…");
        let verifier = RobloxLiveStateVerifier::new(resource_manager.api());
        let (reconciled, report) = reconcile_graph(&current_graph, &verifier).await;
        let counts = report.counts();
        spinner.success(format!(
            "Reconciled with Roblox: {} verified, {} missing, {} skipped, {} unknown",
            counts.verified, counts.missing, counts.skipped, counts.unknown
        ));
        for (id, status) in report.entries.iter() {
            match status {
                VerificationStatus::Missing => logger::log(format!(
                    "{}: missing on Roblox; will be re-created",
                    Paint::yellow(id)
                )),
                VerificationStatus::Unknown(reason) => logger::log(format!(
                    "{}: verification inconclusive ({}); preserving stored state",
                    Paint::yellow(id),
                    reason
                )),
                _ => {}
            }
        }
        (reconciled, report)
    };

    // Pre-apply preview. We derive the plan from the same graphs that
    // `evaluate` is about to act on, so what the user sees and what deploy
    // does cannot drift apart. Confirmation gates any destructive action.
    let rollback_summary = rollback_readiness_summary(&state_config, &current_graph);
    {
        let diff = match next_graph.diff(&current_graph) {
            Ok(d) => d,
            Err(e) => {
                logger::log(Paint::red(format!("Failed to compute plan diff: {}", e)));
                return 1;
            }
        };
        let plan = Plan::build(
            &diff,
            &current_graph,
            &next_graph,
            Some(&reconciliation_report),
            Some(&preflight_report),
            Some(rollback_summary.clone()),
        );

        match preview_and_confirm(&plan, preview_options) {
            Decision::Approve => {}
            Decision::Cancel => {
                logger::log(Paint::yellow("Deploy cancelled by user."));
                return 2;
            }
        }
    }

    logger::start_action("Checkpointing last known good state:");
    let deployment_id = state.begin_deployment(
        &environment_config.label,
        DeploymentKind::Deploy,
        current_graph.get_resource_list(),
        next_graph.get_resource_list(),
        None,
    );
    match save_state(&project_path, &state_config, &state).await {
        Ok(_) => logger::end_action("Succeeded"),
        Err(e) => {
            logger::end_action(Paint::red(e));
            return 1;
        }
    };

    let results = {
        let mut progress_writer = DeploymentProgressWriter::new(
            project_path.as_path(),
            &state_config,
            &mut state,
            &environment_config.label,
            &deployment_id,
            &current_graph,
        );
        next_graph
            .evaluate_with_progress(
                &current_graph,
                &mut resource_manager,
                allow_purchases,
                Some(&mut progress_writer),
            )
            .await
    };
    match &results {
        Ok(results) => {
            match results {
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
            };
        }
        Err(e) => {
            logger::end_action(Paint::red(e));
            if e.applied_mutation_count() > 0 {
                logger::log(Paint::yellow(format!(
                    "Deployment failed after {} applied mutation(s). The environment may be partially updated.",
                    e.applied_mutation_count()
                )));
            }
            logger::log(format!(
                "Recovery: run {} to drive the environment back toward the last known good snapshot.",
                Paint::cyan(format!(
                    "lithos undo --environment {}",
                    environment_config.label
                ))
            ));
        }
    };

    let journal = match &results {
        Ok(_) => build_success_journal(&current_graph, &next_graph),
        Err(error) => build_failure_journal(&current_graph, &next_graph, error),
    };

    let deployment_summary = match &results {
        Ok(EvaluateResults {
            created_count,
            updated_count,
            deleted_count,
            noop_count,
            skipped_count,
        }) => Some(format!(
            "Applied {} create(s), {} update(s), {} delete(s), {} noop(s), {} skip(s)",
            created_count, updated_count, deleted_count, noop_count, skipped_count
        )),
        Err(error) => Some(format!(
            "Failed after {} applied mutation(s) and {} recorded failure(s)",
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
        next_graph.get_resource_list(),
        journal,
        deployment_summary,
    );

    if environment_config.tag_commit && results.is_ok() {
        logger::start_action("Tagging commit:");
        match tag_commit(
            project_path.clone(),
            &target_config,
            &next_graph,
            &current_graph,
        ) {
            Ok(0) => logger::end_action("No tagging required"),
            Ok(tag_count) => {
                logger::end_action(format!("Succeeded in pushing {} tag(s)", tag_count))
            }
            Err(e) => logger::end_action(Paint::red(e)),
        };
    }

    logger::start_action("Saving state:");
    match save_state(&project_path, &state_config, &state).await {
        Ok(_) => {}
        Err(e) => {
            logger::end_action(Paint::red(e));
            return 1;
        }
    };
    logger::end_action("Succeeded");

    log_target_results(&target_config, &next_graph);

    match &results {
        Ok(_) => 0,
        Err(_) => 1,
    }
}

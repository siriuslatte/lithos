use std::{fs, io::IsTerminal, str};

use difference::Changeset;
use yansi::Paint;

use rbx_lithos::{
    config::load_project_config,
    project::{load_project, Project},
    resource_graph::ResourceGraphDiff,
    roblox_resource_manager::RobloxResourceManager,
    state::{get_desired_graph, reconcile_graph, RobloxLiveStateVerifier, VerificationStatus},
};

use crate::preview::{
    model::Plan,
    render::{preview_and_confirm, PreviewMode, PreviewOptions},
};

fn get_changeset(previous_hash: &str, new_hash: &str) -> Changeset {
    Changeset::new(previous_hash, new_hash, "\n")
}

fn print_diff(diff: ResourceGraphDiff) {
    for (resource_id, r) in diff.removals.into_iter() {
        logger::start_action(format!("{} Removed {}:", Paint::red("-"), resource_id));
        logger::log("Inputs:");
        logger::log_changeset(get_changeset(&r.previous_inputs_hash, ""));
        logger::end_action_without_message();
    }

    for (resource_id, r) in diff.additions.into_iter() {
        logger::start_action(format!("{} Added {}:", Paint::green("+"), resource_id));
        logger::log("Inputs:");
        logger::log_changeset(get_changeset("", &r.current_inputs_hash));
        logger::end_action_without_message();
    }

    for (resource_id, r) in diff.changes.into_iter() {
        logger::start_action(format!("{} Changed {}:", Paint::yellow("~"), resource_id));
        logger::log("Inputs:");
        logger::log_changeset(get_changeset(
            &r.previous_inputs_hash,
            &r.current_inputs_hash,
        ));
        logger::end_action_without_message();
    }

    for (resource_id, r) in diff.dependency_changes.into_iter() {
        logger::start_action(format!(
            "{} Dependency Changed {}:",
            Paint::new("○").dimmed(),
            resource_id
        ));
        logger::log("Changed dependencies:");
        for dependency_id in r.changed_dependencies.into_iter() {
            logger::log(format!(
                " {} {}",
                Paint::new("-").dimmed(),
                Paint::yellow(dependency_id)
            ))
        }
        logger::end_action_without_message();
    }
}

pub async fn run(
    project: Option<&str>,
    environment: Option<&str>,
    output: Option<&str>,
    format: Option<&str>,
    live: bool,
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
        target_config,
        owner_config,
        payment_source,
        ..
    } = match load_project(project_path.clone(), config, environment).await {
        Ok(Some(v)) => v,
        Ok(None) => {
            logger::end_action("No diff available");
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

    // Optional live reconciliation. Off by default to keep `diff` fast and
    // network-free; opt-in via `--live` for a Terraform-style plan that
    // surfaces drift between persisted state and real Roblox resources.
    let current_graph = if live {
        logger::start_action("Reconciling state with Roblox:");
        let resource_manager = match RobloxResourceManager::new(&project_path, payment_source).await
        {
            Ok(v) => v,
            Err(e) => {
                logger::end_action(Paint::red(e));
                return 1;
            }
        };
        let verifier = RobloxLiveStateVerifier::new(resource_manager.api());
        let (reconciled, report) = reconcile_graph(&current_graph, &verifier).await;
        let counts = report.counts();
        for (id, status) in report.entries.iter() {
            match status {
                VerificationStatus::Missing => logger::log(format!(
                    "{} {}: drifted (missing on Roblox)",
                    Paint::red("!"),
                    id
                )),
                VerificationStatus::Unknown(reason) => logger::log(format!(
                    "{} {}: verification inconclusive ({})",
                    Paint::yellow("?"),
                    id,
                    reason
                )),
                _ => {}
            }
        }
        logger::end_action(format!(
            "{} verified, {} missing, {} skipped, {} unknown",
            counts.verified, counts.missing, counts.skipped, counts.unknown
        ));
        reconciled
    } else {
        current_graph
    };

    logger::start_action("Diffing resource graphs:");

    let diff = next_graph.diff(&current_graph);

    match diff {
        Ok(diff) => {
            let outputs_string = format.map(|format| match format {
                "json" => serde_json::to_string_pretty(&diff)
                    .map(|x| x + "\n")
                    .map_err(|e| e.to_string()),
                "yaml" => serde_yaml::to_string(&diff).map_err(|e| e.to_string()),
                _ => Err(format!("Unknown format: {}", format)),
            });

            // Pick a renderer:
            //   - machine-readable mode (--format / --output): keep the old
            //     text diff so existing piping behavior is preserved,
            //   - interactive TTY: open the alt-screen plan viewer so users
            //     can navigate the change list with arrow keys,
            //   - otherwise: fall back to the legacy text diff.
            let machine_readable = format.is_some() || output.is_some();
            let interactive = !machine_readable
                && std::io::stdout().is_terminal()
                && std::io::stdin().is_terminal();

            if interactive {
                let plan = Plan::build(&diff, &current_graph, &next_graph, None);
                // `diff` is read-only so we surface the viewer in `Off`-style
                // semantics: the decision is ignored, we just want the user
                // to see and explore the plan.
                let _ = preview_and_confirm(
                    &plan,
                    PreviewOptions {
                        mode: PreviewMode::Auto,
                        // assume_yes short-circuits the destructive
                        // re-confirmation prompt; we don't act on the
                        // result either way.
                        assume_yes: true,
                    },
                );
                logger::end_action("Succeeded");
            } else {
                print_diff(diff);
                logger::end_action("Succeeded");
            }

            if let Some(outputs_string) = outputs_string {
                if let Ok(outputs_string) = outputs_string {
                    if let Some(output) = output {
                        if let Err(e) = fs::write(output, outputs_string).map_err(|e| {
                            format!("Unable to write outputs file: {}\n\t{}", output, e)
                        }) {
                            logger::log(Paint::red(e));
                            return 1;
                        }
                    } else {
                        print!("{}", outputs_string);
                    }
                } else {
                    logger::log(Paint::red("Failed to serialize outputs"));
                    return 1;
                }
            }

            0
        }
        Err(e) => {
            logger::end_action(Paint::red(e));
            1
        }
    }
}

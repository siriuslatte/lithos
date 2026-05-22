use std::str;

use yansi::Paint;

use rbx_lithos::{
    config::load_project_config,
    project::{load_project, Project},
    resource_graph::{EvaluateResults, ResourceGraph},
    roblox_resource_manager::RobloxResourceManager,
    state::{acquire_environment_lock, release_environment_lock, save_state_cas, SaveTarget},
};

pub async fn run(project: Option<&str>, environment: Option<&str>) -> i32 {
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
        mut state_handle,
        environment_config,
        payment_source,
        state_config,
        ..
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
    logger::end_action("Succeeded");

    let lock_session = match acquire_environment_lock(
        &project_path,
        &state_config,
        &mut state,
        &mut state_handle,
        &environment_config.label,
        "destroy",
    )
    .await
    {
        Ok(session) => session,
        Err(e) => {
            logger::log(Paint::red(e));
            return 1;
        }
    };

    logger::start_action("Destroying resources:");
    let mut resource_manager = match RobloxResourceManager::new(&project_path, payment_source).await
    {
        Ok(v) => v,
        Err(e) => {
            logger::end_action(Paint::red(e));
            return 1;
        }
    };

    let mut next_graph = ResourceGraph::new(&Vec::new());
    let results = next_graph
        .evaluate(&current_graph, &mut resource_manager, false)
        .await;
    match &results {
        Ok(results) => {
            match results {
                EvaluateResults {
                    deleted_count: 0, ..
                } => logger::end_action("No changes required"),
                EvaluateResults { deleted_count, .. } => {
                    logger::end_action(format!("Succeeded with {} delete(s)", deleted_count))
                }
            };
        }
        Err(e) => {
            logger::end_action(Paint::red(e));
        }
    };

    logger::start_action("Saving state:");
    let resource_list = next_graph.get_resource_list();
    if resource_list.is_empty() {
        state
            .ensure_environment_mut(&environment_config.label)
            .current = Vec::new();
    } else {
        state
            .ensure_environment_mut(&environment_config.label)
            .current = next_graph.get_resource_list();
    }
    {
        let baseline = state.environment(&environment_config.label).cloned();
        let target = SaveTarget::new(&environment_config.label, baseline);
        match save_state_cas(
            &project_path,
            &state_config,
            &mut state,
            &state_handle,
            &target,
        )
        .await
        {
            Ok(new_handle) => {
                state_handle = new_handle;
            }
            Err(e) => {
                logger::end_action(Paint::red(e));
                let _ = release_environment_lock(
                    &project_path,
                    &state_config,
                    &mut state,
                    &mut state_handle,
                    &lock_session,
                )
                .await;
                return 1;
            }
        }
    }
    logger::end_action("Succeeded");

    if let Err(e) = release_environment_lock(
        &project_path,
        &state_config,
        &mut state,
        &mut state_handle,
        &lock_session,
    )
    .await
    {
        logger::log(Paint::yellow(format!(
            "Warning: failed to release environment lock: {}",
            e
        )));
    }

    match &results {
        Ok(_) => 0,
        Err(_) => 1,
    }
}

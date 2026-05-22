use std::sync::Arc;

use rbx_api::{models::AssetId, RobloxApi};
use rbx_auth::{RobloxCookieStore, RobloxCsrfTokenStore};
use yansi::Paint;

use rbx_lithos::{
    config::load_project_config,
    project::{load_project, Project},
    state::{
        acquire_environment_lock, import_graph, release_environment_lock, save_state_cas,
        SaveTarget,
    },
};

pub async fn run(project: Option<&str>, environment: Option<&str>, target_id: &str) -> i32 {
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
        state_config,
        ..
    } = match load_project(project_path.clone(), config, environment).await {
        Ok(Some(v)) => v,
        Ok(None) => {
            logger::end_action("No import necessary");
            return 0;
        }
        Err(e) => {
            logger::end_action(Paint::red(e));
            return 1;
        }
    };

    if !current_graph.get_resource_list().is_empty() {
        logger::end_action("Environment state already exists: no need to import.");
        return 0;
    }

    logger::end_action("Succeeded");

    let target_id = match target_id.parse::<AssetId>() {
        Ok(v) => v,
        Err(e) => {
            logger::log(Paint::red(format!(
                "Experience ID {} is invalid: {}",
                target_id, e
            )));
            return 1;
        }
    };

    logger::start_action("Import target:");
    let cookie_store = match RobloxCookieStore::new() {
        Ok(v) => Arc::new(v),
        Err(e) => {
            logger::end_action(Paint::red(e));
            return 1;
        }
    };
    let csrf_token_store = RobloxCsrfTokenStore::new();
    let roblox_api = match RobloxApi::new(cookie_store, csrf_token_store, None) {
        Ok(v) => v,
        Err(e) => {
            logger::end_action(Paint::red(e));
            return 1;
        }
    };
    match roblox_api.validate_auth().await {
        Ok(_) => {}
        Err(e) => {
            logger::end_action(Paint::red(e));
            return 1;
        }
    };

    let imported_graph = match import_graph(&roblox_api, target_id).await {
        Ok(v) => v,
        Err(e) => {
            logger::end_action(Paint::red(format!("Failed: {}", e)));
            return 1;
        }
    };
    logger::end_action("Succeeded");

    logger::start_action("Saving state:");
    let lock_session = match acquire_environment_lock(
        &project_path,
        &state_config,
        &mut state,
        &mut state_handle,
        &environment_config.label,
        "import",
    )
    .await
    {
        Ok(session) => session,
        Err(e) => {
            logger::end_action(Paint::red(e));
            return 1;
        }
    };
    state
        .ensure_environment_mut(&environment_config.label)
        .current = imported_graph.get_resource_list();
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

    0
}

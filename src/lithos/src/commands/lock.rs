use yansi::Paint;

use rbx_lithos::{
    config::load_project_config,
    state::{force_break_environment_lock, get_state, EnvironmentLock},
};

/// Print a single lock entry in human-readable form.
fn print_lock(environment: &str, lock: &EnvironmentLock) {
    let stale_marker = if lock.is_stale() {
        Paint::yellow(" (stale)").to_string()
    } else {
        String::new()
    };
    logger::log(format!(
        "{}{}: operation '{}' held by pid {} on {} (acquired {}, heartbeat {})",
        Paint::cyan(environment),
        stale_marker,
        lock.operation,
        lock.pid,
        lock.host,
        lock.acquired_at,
        lock.heartbeat_at,
    ));
}

pub async fn list(project: Option<&str>) -> i32 {
    let (project_path, config) = match load_project_config(project) {
        Ok(v) => v,
        Err(e) => {
            logger::log(Paint::red(e));
            return 1;
        }
    };
    let state = match get_state(&project_path, &config).await {
        Ok(state) => state,
        Err(e) => {
            logger::log(Paint::red(e));
            return 1;
        }
    };
    let locks: Vec<_> = state.iter_environment_locks().collect();
    if locks.is_empty() {
        logger::log("No environment locks are currently held.");
        return 0;
    }
    logger::start_action("Active environment locks:");
    for (env, lock) in locks {
        print_lock(env, lock);
    }
    logger::end_action_without_message();
    0
}

pub async fn break_lock(project: Option<&str>, environment: &str) -> i32 {
    let (project_path, config) = match load_project_config(project) {
        Ok(v) => v,
        Err(e) => {
            logger::log(Paint::red(e));
            return 1;
        }
    };
    match force_break_environment_lock(&project_path, &config.state, environment).await {
        Ok(Some(lock)) => {
            logger::log(format!(
                "Released lock on {} (was held by pid {} on {}, operation '{}', heartbeat {})",
                Paint::cyan(environment),
                lock.pid,
                lock.host,
                lock.operation,
                lock.heartbeat_at,
            ));
            0
        }
        Ok(None) => {
            logger::log(format!(
                "No lock was held on environment {}",
                Paint::cyan(environment)
            ));
            0
        }
        Err(e) => {
            logger::log(Paint::red(e));
            1
        }
    }
}

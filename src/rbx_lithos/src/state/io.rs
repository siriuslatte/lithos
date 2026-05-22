//! State persistence: load and save Lithos resource state to disk or S3.
//!
//! Owns the versioned `ResourceState` enum, file-name conventions,
//! parse/serialize, and the S3 helpers.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use chrono::Utc;
use clap::crate_version;
use rusoto_core::{HttpClient, Region};
use rusoto_s3::{S3Client, S3};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use yansi::Paint;

use crate::config::{Config, EnvironmentConfig, RemoteStateConfig, StateConfig};

use super::{
    aws_credentials_provider::AwsCredentialsProvider,
    store::{build_store, LoadedState, SaveError, StateHandle},
    v1::ResourceStateV1,
    v2::ResourceStateV2,
    v3::ResourceStateV3,
    v4::ResourceStateV4,
    v5::ResourceStateV5,
    v6::ResourceStateV6,
    v7::{EnvironmentStateV7, ResourceStateV7},
};

#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
enum ResourceState {
    Versioned(VersionedResourceState),
    Unversioned(ResourceStateV1),
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "version")]
enum VersionedResourceState {
    #[serde(rename = "1")]
    V1(ResourceStateV1),
    #[serde(rename = "2")]
    V2(ResourceStateV2),
    #[serde(rename = "3")]
    V3(ResourceStateV3),
    #[serde(rename = "4")]
    V4(ResourceStateV4),
    #[serde(rename = "5")]
    V5(ResourceStateV5),
    #[serde(rename = "6")]
    V6(ResourceStateV6),
    #[serde(rename = "7")]
    V7(ResourceStateV7),
}

pub type ResourceStateVLatest = ResourceStateV7;

const STATE_SUFFIX: &str = ".lithos-state.yml";

fn get_state_file_path(project_path: &Path, key: Option<&str>) -> PathBuf {
    project_path.join(format!("{}{}", key.unwrap_or_default(), STATE_SUFFIX))
}

fn get_hash(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    format!("{:x}", digest)
}

pub(super) fn get_file_hash(file_path: PathBuf) -> Result<String, String> {
    let buffer = fs::read(&file_path).map_err(|e| {
        format!(
            "Failed to read file {} for hashing: {}",
            file_path.display(),
            e
        )
    })?;
    Ok(get_hash(&buffer))
}

fn parse_state(file_name: &str, data: &str) -> Result<ResourceState, String> {
    serde_yaml::from_str::<ResourceState>(data)
        .map_err(|e| format!("Unable to parse state file {}\n\t{}", file_name, e))
}

fn create_client(region: Region) -> S3Client {
    S3Client::new_with(
        HttpClient::new().unwrap(),
        AwsCredentialsProvider::new(),
        region,
    )
}

pub async fn get_state_from_source(
    project_path: &Path,
    source: StateConfig,
) -> Result<ResourceStateVLatest, String> {
    let (state, _handle) = load_state_from_source(project_path, &source).await?;
    Ok(state)
}

/// Load the current state together with the [`StateHandle`] identifying the
/// revision it came from. Mutating commands should use this entry point so
/// they can pass the handle back to [`save_state_cas`] for safe writes.
pub async fn load_state_from_source(
    project_path: &Path,
    source: &StateConfig,
) -> Result<(ResourceStateVLatest, StateHandle), String> {
    let store = build_store(project_path, source);
    log_load_source(source);
    let LoadedState { bytes, handle } = store.load().await?;
    let state = match bytes {
        None => ResourceStateVLatest {
            environments: BTreeMap::new(),
            locks: BTreeMap::new(),
        },
        Some(bytes) => parse_state_bytes(&store_label(source), &bytes)?,
    };
    Ok((state, handle))
}

/// Convenience wrapper around [`load_state_from_source`] that uses the state
/// source declared by `config`.
pub async fn load_state_with_handle(
    project_path: &Path,
    config: &Config,
) -> Result<(ResourceStateVLatest, StateHandle), String> {
    load_state_from_source(project_path, &config.state).await
}

fn log_load_source(source: &StateConfig) {
    match source {
        StateConfig::Local => logger::log("Loading previous state from local file".to_owned()),
        StateConfig::LocalKey(key) => logger::log(format!(
            "Loading previous state from local file with key {}",
            Paint::cyan(key)
        )),
        StateConfig::Remote(config) => logger::log(format!(
            "Loading previous state from remote object {}",
            Paint::cyan(config)
        )),
    }
}

fn store_label(source: &StateConfig) -> String {
    match source {
        StateConfig::Local => "<local state>".to_owned(),
        StateConfig::LocalKey(key) => format!("<local state '{}'>", key),
        StateConfig::Remote(config) => format!("{}", config),
    }
}

fn parse_state_bytes(label: &str, bytes: &[u8]) -> Result<ResourceStateVLatest, String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|e| format!("State at {} is not valid UTF-8: {}", label, e))?;
    let parsed = parse_state(label, text)?;
    Ok(migrate_state(parsed))
}

fn migrate_state(state: ResourceState) -> ResourceStateVLatest {
    match state {
        ResourceState::Unversioned(state) => {
            ResourceStateV7::from(ResourceStateV6::from(ResourceStateV5::from(
                ResourceStateV4::from(ResourceStateV3::from(ResourceStateV2::from(state))),
            )))
        }
        ResourceState::Versioned(VersionedResourceState::V1(state)) => {
            ResourceStateV7::from(ResourceStateV6::from(ResourceStateV5::from(
                ResourceStateV4::from(ResourceStateV3::from(ResourceStateV2::from(state))),
            )))
        }
        ResourceState::Versioned(VersionedResourceState::V2(state)) => {
            ResourceStateV7::from(ResourceStateV6::from(ResourceStateV5::from(
                ResourceStateV4::from(ResourceStateV3::from(state)),
            )))
        }
        ResourceState::Versioned(VersionedResourceState::V3(state)) => ResourceStateV7::from(
            ResourceStateV6::from(ResourceStateV5::from(ResourceStateV4::from(state))),
        ),
        ResourceState::Versioned(VersionedResourceState::V4(state)) => {
            ResourceStateV7::from(ResourceStateV6::from(ResourceStateV5::from(state)))
        }
        ResourceState::Versioned(VersionedResourceState::V5(state)) => {
            ResourceStateV7::from(ResourceStateV6::from(state))
        }
        ResourceState::Versioned(VersionedResourceState::V6(state)) => ResourceStateV7::from(state),
        ResourceState::Versioned(VersionedResourceState::V7(state)) => state,
    }
}

pub async fn get_state(
    project_path: &Path,
    config: &Config,
) -> Result<ResourceStateVLatest, String> {
    get_state_from_source(project_path, config.state.clone()).await
}

pub async fn get_previous_state(
    project_path: &Path,
    config: &Config,
    environment_config: &EnvironmentConfig,
) -> Result<ResourceStateVLatest, String> {
    let mut state = get_state(project_path, config).await?;

    if !state.environments.contains_key(&environment_config.label) {
        logger::log(format!(
            "No previous state for environment {}",
            Paint::cyan(environment_config.label.clone())
        ));
        state.environments.insert(
            environment_config.label.clone(),
            super::v7::EnvironmentStateV7::default(),
        );
    }

    Ok(state)
}

pub async fn save_state_to_remote(config: &RemoteStateConfig, data: &[u8]) -> Result<(), String> {
    logger::log(format!("Saving to remote object {}", Paint::cyan(config)));

    let client = create_client(config.region.clone());
    let res = client
        .put_object(rusoto_s3::PutObjectRequest {
            bucket: config.bucket.clone(),
            key: format!("{}.lithos-state.yml", config.key),
            body: Some(rusoto_core::ByteStream::from(data.to_vec())),
            ..Default::default()
        })
        .await;

    res.map(|_| ())
        .map_err(|e| format!("Failed to save state to remote: {}", e))
}

pub fn save_state_to_file(
    project_path: &Path,
    data: &[u8],
    file_path: Option<&str>,
) -> Result<(), String> {
    let state_file_path = get_state_file_path(project_path, file_path);

    logger::log(format!(
        "Saving to local file {}. It is recommended you commit this file to your source control",
        Paint::cyan(state_file_path.display())
    ));

    fs::write(&state_file_path, data).map_err(|e| {
        format!(
            "Unable to write state file: {}\n\t{}",
            state_file_path.display(),
            e
        )
    })?;

    Ok(())
}

fn serialize_state(state: &ResourceStateVLatest) -> Result<Vec<u8>, String> {
    let utc = Utc::now();
    let mut data = format!("#\n\
                                   # WARNING - Generated file. Do not modify directly unless you know \
                                     what you are doing!\n\
                                                                     # This file was generated by Lithos v{} on {}\n\
                                   #\n\n",
                                crate_version!(),
                                utc.format("%FT%TZ")
                            ).as_bytes().to_vec();

    let state_data = serde_yaml::to_vec(&ResourceState::Versioned(VersionedResourceState::V7(
        state.to_owned(),
    )))
    .map_err(|e| format!("Unable to serialize state\n\t{}", e))?;

    data.extend(state_data);

    Ok(data)
}

/// Best-effort save: load the latest revision, then CAS-write on top. On
/// conflict the latest state is silently merged with no special target-env
/// awareness, which is appropriate for whole-file admin commands like
/// `upload` / `download`. Mutating workflows should prefer
/// [`save_state_cas`] so they can describe which environment they own.
pub async fn save_state(
    project_path: &Path,
    state_config: &StateConfig,
    state: &ResourceStateVLatest,
) -> Result<(), String> {
    let store = build_store(project_path, state_config);
    let current = store.load().await?;
    let bytes = serialize_state(state)?;
    log_save_destination(state_config);
    match store.save(&bytes, &current.handle).await {
        Ok(_) => Ok(()),
        Err(SaveError::Other(e)) => Err(e),
        Err(SaveError::Conflict { .. }) => Err(format!(
            "Concurrent write detected while saving {}. Reload the state and retry.",
            store.describe()
        )),
    }
}

/// Describes which environment a save is authoritatively updating, plus the
/// snapshot of that environment as it existed at the start of the operation.
/// Used by [`save_state_cas`] to distinguish "another writer touched a
/// different environment" (merge and retry) from "another writer touched the
/// same environment we are mutating" (fail fast).
pub struct SaveTarget<'a> {
    pub environment: &'a str,
    pub baseline: Option<EnvironmentStateV7>,
}

impl<'a> SaveTarget<'a> {
    pub fn new(environment: &'a str, baseline: Option<EnvironmentStateV7>) -> Self {
        Self {
            environment,
            baseline,
        }
    }
}

/// Compare-and-swap save with per-environment conflict resolution.
///
/// `expected` must be the handle returned from the most recent successful
/// load/save in this operation. `target` declares which environment this
/// operation owns; on a CAS conflict:
///
/// - If the on-disk state's view of `target.environment` matches `target.baseline`,
///   the conflict only touched *other* environments. We rebase by taking the
///   on-disk state and overlaying our environment's slice on top, then retry.
/// - Otherwise the conflict touched the same environment we own, which is
///   not a safe state to recover from automatically — we return an error and
///   the caller should surface it to the user.
///
/// On success `state` is updated in place so subsequent in-memory mutations
/// build on the latest disk state, and the new [`StateHandle`] is returned.
pub async fn save_state_cas(
    project_path: &Path,
    state_config: &StateConfig,
    state: &mut ResourceStateVLatest,
    expected: &StateHandle,
    target: &SaveTarget<'_>,
) -> Result<StateHandle, String> {
    let store = build_store(project_path, state_config);
    const MAX_RETRIES: u32 = 3;
    let mut current_expected = expected.clone();
    log_save_destination(state_config);

    for attempt in 0..=MAX_RETRIES {
        let bytes = serialize_state(state)?;
        match store.save(&bytes, &current_expected).await {
            Ok(new_handle) => return Ok(new_handle),
            Err(SaveError::Other(e)) => return Err(e),
            Err(SaveError::Conflict { latest }) => {
                if attempt == MAX_RETRIES {
                    return Err(format!(
                        "Repeated concurrent writes to {} prevented a safe save after {} retries.",
                        store.describe(),
                        MAX_RETRIES
                    ));
                }
                let latest_state = match latest.bytes.as_deref() {
                    Some(bytes) => parse_state_bytes(&store_label(state_config), bytes)?,
                    None => ResourceStateVLatest {
                        environments: BTreeMap::new(),
                        locks: BTreeMap::new(),
                    },
                };
                rebase_onto_latest(state, latest_state, target)?;
                current_expected = latest.handle;
            }
        }
    }
    Err(format!(
        "Unable to safely save state to {}",
        store.describe()
    ))
}

fn log_save_destination(state_config: &StateConfig) {
    match state_config {
        StateConfig::Local => {
            logger::log("Saving state to local file".to_owned());
        }
        StateConfig::LocalKey(key) => logger::log(format!(
            "Saving state to local file with key {}",
            Paint::cyan(key)
        )),
        StateConfig::Remote(config) => {
            logger::log(format!(
                "Saving state to remote object {}",
                Paint::cyan(config)
            ));
        }
    }
}

/// Reconcile our in-memory state with the latest on-disk state when only
/// other environments changed. Returns an error if our owned environment is
/// no longer the one we started from (a same-environment conflict).
fn rebase_onto_latest(
    state: &mut ResourceStateVLatest,
    latest: ResourceStateVLatest,
    target: &SaveTarget<'_>,
) -> Result<(), String> {
    let target_label = target.environment;
    let latest_target = latest.environments.get(target_label);

    let same_env_conflict = match (target.baseline.as_ref(), latest_target) {
        (Some(baseline), Some(remote)) => !environments_equivalent(baseline, remote),
        (Some(_), None) => true,
        (None, Some(_)) => true,
        (None, None) => false,
    };

    if same_env_conflict {
        return Err(format!(
            "Concurrent change detected for environment '{}'. Another writer modified the same \
             environment while this operation was in progress; refusing to overwrite their \
             changes. Re-run the command once the other operation has finished.",
            target_label
        ));
    }

    // Take everything from the latest snapshot and overlay our target env.
    let our_env = state.environments.remove(target_label);
    let mut merged = latest;
    if let Some(our_env) = our_env {
        merged.environments.insert(target_label.to_owned(), our_env);
    } else {
        merged.environments.remove(target_label);
    }
    *state = merged;
    Ok(())
}

fn environments_equivalent(a: &EnvironmentStateV7, b: &EnvironmentStateV7) -> bool {
    // Cheap structural equality via YAML serialization. The state types do
    // not derive `PartialEq`, and adding it transitively across the resource
    // graph types would be a much larger surface change.
    match (serde_yaml::to_string(a), serde_yaml::to_string(b)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        resource_graph::Resource,
        roblox_resource_manager::{ExperienceInputs, RobloxInputs, RobloxResource},
    };
    use tempfile::TempDir;

    fn make_resource(id: &str) -> RobloxResource {
        RobloxResource::new(
            id,
            RobloxInputs::Experience(ExperienceInputs { group_id: None }),
            &[],
        )
    }

    fn state_with_env(label: &str, marker: &str) -> ResourceStateVLatest {
        let mut state = ResourceStateVLatest {
            environments: BTreeMap::new(),
            locks: BTreeMap::new(),
        };
        state.environments.insert(
            label.to_owned(),
            EnvironmentStateV7 {
                current: vec![make_resource(marker)],
                deployments: Vec::new(),
            },
        );
        state
    }

    #[tokio::test]
    async fn save_state_cas_merges_when_other_env_changed() {
        let dir = TempDir::new().unwrap();
        let cfg = StateConfig::Local;

        // Seed: state contains both environments.
        let mut seed = ResourceStateVLatest {
            environments: BTreeMap::new(),
            locks: BTreeMap::new(),
        };
        seed.environments.insert(
            "dev".to_owned(),
            EnvironmentStateV7 {
                current: vec![make_resource("dev-resource")],
                deployments: Vec::new(),
            },
        );
        seed.environments.insert(
            "prod".to_owned(),
            EnvironmentStateV7 {
                current: vec![make_resource("prod-resource")],
                deployments: Vec::new(),
            },
        );
        save_state(dir.path(), &cfg, &seed).await.unwrap();

        // Operation A loads state and prepares to update "prod".
        let (mut state_a, handle_a) = load_state_from_source(dir.path(), &cfg).await.unwrap();
        let baseline_prod = state_a.environment("prod").cloned();
        state_a.ensure_environment_mut("prod").current = vec![make_resource("prod-resource-v2")];

        // Operation B sneaks in a write to "dev" first.
        let (mut state_b, handle_b) = load_state_from_source(dir.path(), &cfg).await.unwrap();
        let baseline_dev = state_b.environment("dev").cloned();
        state_b.ensure_environment_mut("dev").current = vec![make_resource("dev-resource-v2")];
        save_state_cas(
            dir.path(),
            &cfg,
            &mut state_b,
            &handle_b,
            &SaveTarget::new("dev", baseline_dev),
        )
        .await
        .unwrap();

        // Operation A saves: should detect the conflict, merge dev from disk,
        // and commit prod.
        save_state_cas(
            dir.path(),
            &cfg,
            &mut state_a,
            &handle_a,
            &SaveTarget::new("prod", baseline_prod),
        )
        .await
        .expect("merge save should succeed");

        let (final_state, _) = load_state_from_source(dir.path(), &cfg).await.unwrap();
        assert_eq!(
            final_state.environment("prod").unwrap().current[0].get_id(),
            "prod-resource-v2",
            "prod should reflect operation A"
        );
        assert_eq!(
            final_state.environment("dev").unwrap().current[0].get_id(),
            "dev-resource-v2",
            "dev should reflect operation B"
        );
    }

    #[tokio::test]
    async fn save_state_cas_rejects_same_env_conflict() {
        let dir = TempDir::new().unwrap();
        let cfg = StateConfig::Local;
        save_state(dir.path(), &cfg, &state_with_env("prod", "r0"))
            .await
            .unwrap();

        let (mut state_a, handle_a) = load_state_from_source(dir.path(), &cfg).await.unwrap();
        let baseline_a = state_a.environment("prod").cloned();
        state_a.ensure_environment_mut("prod").current = vec![make_resource("r1-A")];

        // Operation B updates prod first.
        let (mut state_b, handle_b) = load_state_from_source(dir.path(), &cfg).await.unwrap();
        let baseline_b = state_b.environment("prod").cloned();
        state_b.ensure_environment_mut("prod").current = vec![make_resource("r1-B")];
        save_state_cas(
            dir.path(),
            &cfg,
            &mut state_b,
            &handle_b,
            &SaveTarget::new("prod", baseline_b),
        )
        .await
        .unwrap();

        let err = save_state_cas(
            dir.path(),
            &cfg,
            &mut state_a,
            &handle_a,
            &SaveTarget::new("prod", baseline_a),
        )
        .await
        .expect_err("same-env conflict must be rejected");
        assert!(
            err.contains("environment 'prod'"),
            "error should mention the contended environment, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn save_state_cas_succeeds_on_clean_handle() {
        let dir = TempDir::new().unwrap();
        let cfg = StateConfig::Local;
        let mut state = state_with_env("prod", "r0");
        save_state(dir.path(), &cfg, &state).await.unwrap();

        let (mut loaded, handle) = load_state_from_source(dir.path(), &cfg).await.unwrap();
        let baseline = loaded.environment("prod").cloned();
        loaded.ensure_environment_mut("prod").current = vec![make_resource("r1")];
        // ensure original `state` binding is silenced if unused
        state = loaded;
        let _new_handle = save_state_cas(
            dir.path(),
            &cfg,
            &mut state,
            &handle,
            &SaveTarget::new("prod", baseline),
        )
        .await
        .unwrap();

        let (after, _) = load_state_from_source(dir.path(), &cfg).await.unwrap();
        assert_eq!(after.environment("prod").unwrap().current[0].get_id(), "r1");
    }
}

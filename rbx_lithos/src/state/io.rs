//! State persistence: load and save Lithos resource state to disk or S3.
//!
//! Owns the versioned `ResourceState` enum, file-name conventions (with
//! Mantle legacy fallbacks), parse/serialize, and the S3 helpers.

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
use tokio::io::AsyncReadExt;
use yansi::Paint;

use crate::config::{Config, EnvironmentConfig, RemoteStateConfig, StateConfig};

use super::{
    aws_credentials_provider::AwsCredentialsProvider, v1::ResourceStateV1, v2::ResourceStateV2,
    v3::ResourceStateV3, v4::ResourceStateV4, v5::ResourceStateV5, v6::ResourceStateV6,
    v7::ResourceStateV7,
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
const LEGACY_STATE_SUFFIX: &str = ".mantle-state.yml";

fn get_state_file_path(project_path: &Path, key: Option<&str>) -> PathBuf {
    project_path.join(format!("{}{}", key.unwrap_or_default(), STATE_SUFFIX))
}

fn get_legacy_state_file_path(project_path: &Path, key: Option<&str>) -> PathBuf {
    project_path.join(format!(
        "{}{}",
        key.unwrap_or_default(),
        LEGACY_STATE_SUFFIX
    ))
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

fn get_state_from_file(
    project_path: &Path,
    key: Option<&str>,
) -> Result<Option<ResourceState>, String> {
    let state_file_path = get_state_file_path(project_path, key);
    let legacy_state_file_path = get_legacy_state_file_path(project_path, key);

    // Prefer the Lithos-named state file. Fall back to the legacy Mantle name
    // for backward compatibility; on next save we'll write to the new name.
    let resolved_path = if state_file_path.exists() {
        state_file_path
    } else if legacy_state_file_path.exists() {
        logger::log(format!(
            "{} Loading legacy state file '{}'. It will be migrated to '{}' on next save.",
            Paint::yellow("warning:"),
            legacy_state_file_path.display(),
            get_state_file_path(project_path, key).display()
        ));
        legacy_state_file_path
    } else {
        logger::log(format!(
            "Loading previous state from local file {}",
            Paint::cyan(state_file_path.display())
        ));
        return Ok(None);
    };

    logger::log(format!(
        "Loading previous state from local file {}",
        Paint::cyan(resolved_path.display())
    ));

    let data = fs::read_to_string(&resolved_path).map_err(|e| {
        format!(
            "Unable to read state file: {}\n\t{}",
            resolved_path.display(),
            e
        )
    })?;

    Ok(Some(parse_state(
        &resolved_path.display().to_string(),
        &data,
    )?))
}

fn create_client(region: Region) -> S3Client {
    S3Client::new_with(
        HttpClient::new().unwrap(),
        AwsCredentialsProvider::new(),
        region,
    )
}

async fn get_state_from_remote(
    config: &RemoteStateConfig,
) -> Result<Option<ResourceState>, String> {
    logger::log(format!(
        "Loading previous state from remote object {}",
        Paint::cyan(config)
    ));

    let client = create_client(config.region.clone());

    // Try the new Lithos-named key first, then fall back to the legacy Mantle name.
    let keys = [
        (format!("{}.lithos-state.yml", config.key), false),
        (format!("{}.mantle-state.yml", config.key), true),
    ];

    for (key, is_legacy) in keys.iter() {
        let object_res = client
            .get_object(rusoto_s3::GetObjectRequest {
                bucket: config.bucket.clone(),
                key: key.clone(),
                ..Default::default()
            })
            .await;

        match object_res {
            Ok(object) => {
                if *is_legacy {
                    logger::log(format!(
                        "{} Loaded legacy remote state object '{}'. It will be migrated to '{}.lithos-state.yml' on next save.",
                        Paint::yellow("warning:"),
                        key,
                        config.key
                    ));
                }
                if let Some(stream) = object.body {
                    let mut buffer = String::new();
                    stream
                        .into_async_read()
                        .read_to_string(&mut buffer)
                        .await
                        .map_err(|_| "".to_owned())?;
                    return Ok(Some(parse_state(&format!("{}", config), &buffer)?));
                }
                return Ok(None);
            }
            Err(rusoto_core::RusotoError::Service(rusoto_s3::GetObjectError::NoSuchKey(_))) => {
                continue
            }
            Err(e) => return Err(format!("Failed to get state from remote: {}", e)),
        }
    }

    Ok(None)
}

pub async fn get_state_from_source(
    project_path: &Path,
    source: StateConfig,
) -> Result<ResourceStateVLatest, String> {
    let state = match source {
        StateConfig::Local => get_state_from_file(project_path, None)?,
        StateConfig::LocalKey(key) => get_state_from_file(project_path, Some(&key))?,
        StateConfig::Remote(config) => get_state_from_remote(&config).await?,
    };

    // Migrate previous state formats
    Ok(match state {
        Some(ResourceState::Unversioned(state)) => {
            ResourceStateV7::from(ResourceStateV6::from(ResourceStateV5::from(
                ResourceStateV4::from(ResourceStateV3::from(ResourceStateV2::from(state))),
            )))
        }
        Some(ResourceState::Versioned(VersionedResourceState::V1(state))) => {
            ResourceStateV7::from(ResourceStateV6::from(ResourceStateV5::from(
                ResourceStateV4::from(ResourceStateV3::from(ResourceStateV2::from(state))),
            )))
        }
        Some(ResourceState::Versioned(VersionedResourceState::V2(state))) => {
            ResourceStateV7::from(ResourceStateV6::from(ResourceStateV5::from(
                ResourceStateV4::from(ResourceStateV3::from(state)),
            )))
        }
        Some(ResourceState::Versioned(VersionedResourceState::V3(state))) => ResourceStateV7::from(
            ResourceStateV6::from(ResourceStateV5::from(ResourceStateV4::from(state))),
        ),
        Some(ResourceState::Versioned(VersionedResourceState::V4(state))) => {
            ResourceStateV7::from(ResourceStateV6::from(ResourceStateV5::from(state)))
        }
        Some(ResourceState::Versioned(VersionedResourceState::V5(state))) => {
            ResourceStateV7::from(ResourceStateV6::from(state))
        }
        Some(ResourceState::Versioned(VersionedResourceState::V6(state))) => {
            ResourceStateV7::from(state)
        }
        Some(ResourceState::Versioned(VersionedResourceState::V7(state))) => state,
        None => ResourceStateVLatest {
            environments: BTreeMap::new(),
        },
    })
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
                                   # This file was generated by Mantle v{} on {}\n\
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

pub async fn save_state(
    project_path: &Path,
    state_config: &StateConfig,
    state: &ResourceStateVLatest,
) -> Result<(), String> {
    let data = serialize_state(state)?;

    match state_config {
        StateConfig::Local => save_state_to_file(project_path, &data, None),
        StateConfig::LocalKey(key) => save_state_to_file(project_path, &data, Some(key)),
        StateConfig::Remote(config) => save_state_to_remote(config, &data).await,
    }
}

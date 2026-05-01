//! Roblox resource manager module.
//!
//! This module is organized into focused submodules:
//!
//! - [`inputs`] – pure data types describing desired state.
//! - [`outputs`] – pure data types describing realized state.
//! - [`resource`] – the [`RobloxResource`] graph node and its
//!   [`Resource`](crate::resource_graph::Resource) implementation.
//! - [`quota`] – pure formatting helpers for quota messaging.
//! - [`ops`] – the per-operation dispatchers (`create` / `update` / `delete`)
//!   that perform Roblox API side effects.
//!
//! The root file (this one) hosts [`RobloxResourceManager`], which owns the
//! HTTP / Open Cloud clients and authenticated user. The
//! [`ResourceManager`](crate::resource_graph::ResourceManager) impl here is a
//! thin orchestration layer that delegates to [`ops`].

use std::{
    env,
    path::{Path, PathBuf},
    sync::Arc,
};

use async_trait::async_trait;
use log::info;
use rbx_api::{
    api_keys::models::IntrospectApiKeyResponse, errors::RobloxApiError, models::CreatorType,
    user::models::GetAuthenticatedUserResponse, RobloxApi,
};
use rbx_auth::{RobloxCookieStore, RobloxCsrfTokenStore};
use rbxcloud::rbx::v1::RbxCloud;
use yansi::Paint;

use crate::diagnostics::{
    map_operation_message, map_roblox_api_error, OperationAction, OperationContext, OperationError,
};
use crate::resource_graph::ResourceManager;

pub mod inputs;
mod ops;
pub mod outputs;
mod quota;
pub mod resource;

pub use inputs::{
    AssetAliasInputs, BadgeInputs, ExperienceActivationInputs, ExperienceInputs, FileInputs,
    FileWithGroupIdInputs, NotificationInputs, PassInputs, PlaceInputs, ProductInputs,
    RobloxInputs, SocialLinkInputs, SpatialVoiceInputs,
};
pub use outputs::{
    AssetAliasOutputs, AssetOutputs, AssetWithInitialIconOutputs, ExperienceOutputs,
    ImageAssetOutputs, NotificationOutputs, PassOutputs, PlaceFileOutputs, ProductOutputs,
    RobloxOutputs,
};
pub use resource::RobloxResource;

pub struct RobloxResourceManager {
    pub(super) roblox_api: RobloxApi,
    pub(super) roblox_cloud: Option<RbxCloud>,
    pub(super) open_cloud_api_key: Option<String>,
    pub(super) project_path: PathBuf,
    pub(super) payment_source: CreatorType,
    pub(super) user: GetAuthenticatedUserResponse,
}

fn load_open_cloud_api_key() -> Option<String> {
    for (name, value) in [
        (
            "LITHOS_OPEN_CLOUD_API_KEY",
            env::var("LITHOS_OPEN_CLOUD_API_KEY").ok(),
        ),
        (
            "ROBLOX_OPEN_CLOUD_API_KEY",
            env::var("ROBLOX_OPEN_CLOUD_API_KEY").ok(),
        ),
        (
            "MANTLE_OPEN_CLOUD_API_KEY",
            env::var("MANTLE_OPEN_CLOUD_API_KEY").ok(),
        ),
    ] {
        if let Some(value) = value {
            info!("Loaded Open Cloud API key from {}.", name);
            return Some(value);
        }
    }

    None
}

impl RobloxResourceManager {
    pub async fn new(project_path: &Path, payment_source: CreatorType) -> Result<Self, String> {
        let open_cloud_api_key = load_open_cloud_api_key();

        let cookie_store = Arc::new(RobloxCookieStore::new()?);
        let csrf_token_store = RobloxCsrfTokenStore::new();
        let roblox_api =
            RobloxApi::new(cookie_store, csrf_token_store, open_cloud_api_key.clone())?;

        logger::start_action("Logging in:");
        let user = match roblox_api.get_authenticated_user().await {
            Ok(user) => {
                logger::log(format!("User ID: {}", user.id));
                logger::log(format!("User name: {}", user.name));
                logger::log(format!("User display name: {}", user.display_name));
                logger::end_action_without_message();
                user
            }
            Err(err) => {
                return {
                    logger::log(Paint::red("Failed to login"));
                    logger::end_action_without_message();
                    Err(err.into())
                }
            }
        };

        let roblox_cloud = open_cloud_api_key
            .as_ref()
            .map(|api_key| RbxCloud::new(api_key));

        Ok(Self {
            roblox_api,
            roblox_cloud,
            open_cloud_api_key,
            project_path: project_path.to_path_buf(),
            payment_source,
            user,
        })
    }

    pub(super) fn get_path<S: Into<String>>(&self, file: S) -> PathBuf {
        self.project_path.join(file.into())
    }

    /// Borrow the underlying Roblox API client. Used by live state
    /// reconciliation, which needs to verify persisted resources against
    /// the live Roblox platform without going through the full
    /// create/update/delete dispatch surface.
    pub fn api(&self) -> &RobloxApi {
        &self.roblox_api
    }

    pub fn has_open_cloud_api_key(&self) -> bool {
        self.open_cloud_api_key.is_some()
    }

    pub async fn introspect_open_cloud_api_key(
        &self,
    ) -> Result<Option<IntrospectApiKeyResponse>, RobloxApiError> {
        let Some(api_key) = self.open_cloud_api_key.as_ref() else {
            return Ok(None);
        };

        self.roblox_api.introspect_api_key(api_key).await.map(Some)
    }

    pub fn context_for_inputs(
        &self,
        action: OperationAction,
        resource_id: &str,
        inputs: &RobloxInputs,
    ) -> OperationContext {
        OperationContext {
            action,
            resource_id: Some(resource_id.to_owned()),
            resource_type: resource_type_for_inputs(inputs).to_owned(),
            auth_model: auth_model_for_inputs(self, inputs).to_owned(),
            creator_target: creator_target_for_inputs(inputs),
            endpoint: None,
            file_path: file_path_for_inputs(inputs),
        }
    }

    pub fn context_for_outputs(
        &self,
        action: OperationAction,
        resource_id: &str,
        outputs: &RobloxOutputs,
    ) -> OperationContext {
        OperationContext {
            action,
            resource_id: Some(resource_id.to_owned()),
            resource_type: resource_type_for_outputs(outputs).to_owned(),
            auth_model: auth_model_for_outputs(self, outputs).to_owned(),
            creator_target: None,
            endpoint: None,
            file_path: None,
        }
    }

    pub fn wrap_api_result<T>(
        &self,
        context: OperationContext,
        result: Result<T, RobloxApiError>,
    ) -> Result<T, OperationError> {
        result.map_err(|error| map_roblox_api_error(context, &error))
    }

    pub fn wrap_message_result<T>(
        &self,
        context: OperationContext,
        result: Result<T, String>,
    ) -> Result<T, OperationError> {
        result.map_err(|message| map_operation_message(context, &message))
    }

    pub fn operation_error(
        &self,
        context: OperationContext,
        message: impl Into<String>,
    ) -> OperationError {
        let message = message.into();
        map_operation_message(context, &message)
    }
}

#[async_trait]
impl ResourceManager<RobloxInputs, RobloxOutputs> for RobloxResourceManager {
    async fn get_create_price(
        &self,
        resource_id: &str,
        inputs: RobloxInputs,
        dependency_outputs: Vec<RobloxOutputs>,
    ) -> Result<Option<u32>, OperationError> {
        ops::price::get_create_price(self, resource_id, inputs, dependency_outputs).await
    }

    async fn create(
        &self,
        resource_id: &str,
        inputs: RobloxInputs,
        dependency_outputs: Vec<RobloxOutputs>,
        price: Option<u32>,
    ) -> Result<RobloxOutputs, OperationError> {
        ops::create::create(self, resource_id, inputs, dependency_outputs, price).await
    }

    async fn get_update_price(
        &self,
        _resource_id: &str,
        _inputs: RobloxInputs,
        _outputs: RobloxOutputs,
        _dependency_outputs: Vec<RobloxOutputs>,
    ) -> Result<Option<u32>, OperationError> {
        Ok(None)
    }

    async fn update(
        &self,
        resource_id: &str,
        inputs: RobloxInputs,
        outputs: RobloxOutputs,
        dependency_outputs: Vec<RobloxOutputs>,
        price: Option<u32>,
    ) -> Result<RobloxOutputs, OperationError> {
        ops::update::update(
            self,
            resource_id,
            inputs,
            outputs,
            dependency_outputs,
            price,
        )
        .await
    }

    async fn delete(
        &self,
        resource_id: &str,
        outputs: RobloxOutputs,
        dependency_outputs: Vec<RobloxOutputs>,
    ) -> Result<(), OperationError> {
        ops::delete::delete(self, resource_id, outputs, dependency_outputs).await
    }
}

fn resource_type_for_inputs(inputs: &RobloxInputs) -> &'static str {
    match inputs {
        RobloxInputs::Experience(_) => "Experience",
        RobloxInputs::ExperienceConfiguration(_) => "ExperienceConfiguration",
        RobloxInputs::ExperienceActivation(_) => "ExperienceActivation",
        RobloxInputs::ExperienceIcon(_) => "ExperienceIcon",
        RobloxInputs::ExperienceThumbnail(_) => "ExperienceThumbnail",
        RobloxInputs::ExperienceThumbnailOrder => "ExperienceThumbnailOrder",
        RobloxInputs::Place(_) => "Place",
        RobloxInputs::PlaceFile(_) => "PlaceFile",
        RobloxInputs::PlaceConfiguration(_) => "PlaceConfiguration",
        RobloxInputs::SocialLink(_) => "SocialLink",
        RobloxInputs::Product(_) => "Product",
        RobloxInputs::ProductIcon(_) => "ProductIcon",
        RobloxInputs::Pass(_) => "Pass",
        RobloxInputs::Badge(_) => "Badge",
        RobloxInputs::BadgeIcon(_) => "BadgeIcon",
        RobloxInputs::ImageAsset(_) => "ImageAsset",
        RobloxInputs::AudioAsset(_) => "AudioAsset",
        RobloxInputs::AssetAlias(_) => "AssetAlias",
        RobloxInputs::SpatialVoice(_) => "SpatialVoice",
        RobloxInputs::Notification(_) => "Notification",
    }
}

fn resource_type_for_outputs(outputs: &RobloxOutputs) -> &'static str {
    match outputs {
        RobloxOutputs::Experience(_) => "Experience",
        RobloxOutputs::ExperienceConfiguration => "ExperienceConfiguration",
        RobloxOutputs::ExperienceActivation => "ExperienceActivation",
        RobloxOutputs::ExperienceIcon(_) => "ExperienceIcon",
        RobloxOutputs::ExperienceThumbnail(_) => "ExperienceThumbnail",
        RobloxOutputs::ExperienceThumbnailOrder => "ExperienceThumbnailOrder",
        RobloxOutputs::Place(_) => "Place",
        RobloxOutputs::PlaceFile(_) => "PlaceFile",
        RobloxOutputs::PlaceConfiguration => "PlaceConfiguration",
        RobloxOutputs::SocialLink(_) => "SocialLink",
        RobloxOutputs::Product(_) => "Product",
        RobloxOutputs::ProductIcon(_) => "ProductIcon",
        RobloxOutputs::Pass(_) => "Pass",
        RobloxOutputs::Badge(_) => "Badge",
        RobloxOutputs::BadgeIcon(_) => "BadgeIcon",
        RobloxOutputs::ImageAsset(_) => "ImageAsset",
        RobloxOutputs::AudioAsset(_) => "AudioAsset",
        RobloxOutputs::AssetAlias(_) => "AssetAlias",
        RobloxOutputs::SpatialVoice => "SpatialVoice",
        RobloxOutputs::Notification(_) => "Notification",
    }
}

fn file_path_for_inputs(inputs: &RobloxInputs) -> Option<String> {
    match inputs {
        RobloxInputs::ExperienceIcon(file)
        | RobloxInputs::ExperienceThumbnail(file)
        | RobloxInputs::PlaceFile(file)
        | RobloxInputs::ProductIcon(file)
        | RobloxInputs::BadgeIcon(file) => Some(file.file_path.clone()),
        RobloxInputs::Pass(inputs) => Some(inputs.icon_file_path.clone()),
        RobloxInputs::Badge(inputs) => Some(inputs.icon_file_path.clone()),
        RobloxInputs::ImageAsset(file) | RobloxInputs::AudioAsset(file) => {
            Some(file.file_path.clone())
        }
        _ => None,
    }
}

fn creator_target_for_inputs(inputs: &RobloxInputs) -> Option<String> {
    match inputs {
        RobloxInputs::Experience(inputs) => inputs
            .group_id
            .map(|group_id| format!("group:{}", group_id)),
        RobloxInputs::ImageAsset(inputs) | RobloxInputs::AudioAsset(inputs) => inputs
            .group_id
            .map(|group_id| format!("group:{}", group_id)),
        _ => None,
    }
}

fn auth_model_for_inputs(mgr: &RobloxResourceManager, inputs: &RobloxInputs) -> &'static str {
    match inputs {
        RobloxInputs::PlaceFile(_) => {
            if mgr.has_open_cloud_api_key() {
                "open-cloud"
            } else {
                "missing-open-cloud"
            }
        }
        RobloxInputs::ImageAsset(_) => {
            if mgr.has_open_cloud_api_key() {
                "hybrid"
            } else {
                "cookie-session"
            }
        }
        _ => "cookie-session",
    }
}

fn auth_model_for_outputs(mgr: &RobloxResourceManager, outputs: &RobloxOutputs) -> &'static str {
    match outputs {
        RobloxOutputs::PlaceFile(_) => {
            if mgr.has_open_cloud_api_key() {
                "open-cloud"
            } else {
                "missing-open-cloud"
            }
        }
        _ => "cookie-session",
    }
}

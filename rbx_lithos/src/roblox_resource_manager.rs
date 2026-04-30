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
use rbx_api::{models::CreatorType, user::models::GetAuthenticatedUserResponse, RobloxApi};
use rbx_auth::{RobloxCookieStore, RobloxCsrfTokenStore};
use rbxcloud::rbx::v1::RbxCloud;
use yansi::Paint;

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
    pub(super) project_path: PathBuf,
    pub(super) payment_source: CreatorType,
    pub(super) user: GetAuthenticatedUserResponse,
}

impl RobloxResourceManager {
    pub async fn new(project_path: &Path, payment_source: CreatorType) -> Result<Self, String> {
        let open_cloud_api_key = match env::var("ROBLOX_OPEN_CLOUD_API_KEY") {
            Ok(v) => {
                info!("Loaded Open Cloud API key from ROBLOX_OPEN_CLOUD_API_KEY.");
                Some(v)
            }
            Err(_) => None,
        };

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

        let roblox_cloud = open_cloud_api_key.map(|api_key| RbxCloud::new(&api_key));

        Ok(Self {
            roblox_api,
            roblox_cloud,
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
}

#[async_trait]
impl ResourceManager<RobloxInputs, RobloxOutputs> for RobloxResourceManager {
    async fn get_create_price(
        &self,
        inputs: RobloxInputs,
        dependency_outputs: Vec<RobloxOutputs>,
    ) -> Result<Option<u32>, String> {
        ops::price::get_create_price(self, inputs, dependency_outputs).await
    }

    async fn create(
        &self,
        inputs: RobloxInputs,
        dependency_outputs: Vec<RobloxOutputs>,
        price: Option<u32>,
    ) -> Result<RobloxOutputs, String> {
        ops::create::create(self, inputs, dependency_outputs, price).await
    }

    async fn get_update_price(
        &self,
        _inputs: RobloxInputs,
        _outputs: RobloxOutputs,
        _dependency_outputs: Vec<RobloxOutputs>,
    ) -> Result<Option<u32>, String> {
        Ok(None)
    }

    async fn update(
        &self,
        inputs: RobloxInputs,
        outputs: RobloxOutputs,
        dependency_outputs: Vec<RobloxOutputs>,
        price: Option<u32>,
    ) -> Result<RobloxOutputs, String> {
        ops::update::update(self, inputs, outputs, dependency_outputs, price).await
    }

    async fn delete(
        &self,
        outputs: RobloxOutputs,
        dependency_outputs: Vec<RobloxOutputs>,
    ) -> Result<(), String> {
        ops::delete::delete(self, outputs, dependency_outputs).await
    }
}

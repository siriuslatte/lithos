//! `update` dispatcher.
//!
//! Many resources have no real "update" – they delete + create or simply
//! recreate. The remaining cases issue a targeted Roblox API call to mutate
//! the existing entity.

use rbx_api::{
    asset_permissions::models::{
        GrantAssetPermissionRequestAction, GrantAssetPermissionRequestSubjectType,
        GrantAssetPermissionsRequestRequest,
    },
    game_passes::models::GetGamePassResponse,
    models::UploadImageResponse,
    spatial_voice::models::UpdateSpatialVoiceSettingsRequest,
};

use crate::diagnostics::{OperationAction, OperationError};
use crate::resource_graph::{all_outputs, optional_output, single_output};

use super::super::{
    inputs::RobloxInputs,
    outputs::{AssetAliasOutputs, AssetOutputs, PassOutputs, RobloxOutputs},
    RobloxResourceManager,
};

macro_rules! api_call {
    ($mgr:expr, $context:expr, $endpoint:expr, $call:expr) => {{
        $mgr.wrap_api_result($context.clone().with_endpoint($endpoint), ($call).await)?
    }};
}

// TODO: Consider moving `outputs` into `dependency_outputs`.
pub(in crate::roblox_resource_manager) async fn update(
    mgr: &RobloxResourceManager,
    resource_id: &str,
    inputs: RobloxInputs,
    outputs: RobloxOutputs,
    dependency_outputs: Vec<RobloxOutputs>,
    price: Option<u32>,
) -> Result<RobloxOutputs, OperationError> {
    let context = mgr.context_for_inputs(OperationAction::Update, resource_id, &inputs);

    match (inputs.clone(), outputs.clone()) {
        (RobloxInputs::Experience(_), RobloxOutputs::Experience(_)) => {
            super::delete::delete(mgr, resource_id, outputs, dependency_outputs.clone()).await?;
            super::create::create(mgr, resource_id, inputs, dependency_outputs, price).await
        }
        (RobloxInputs::ExperienceConfiguration(_), RobloxOutputs::ExperienceConfiguration) => {
            super::create::create(mgr, resource_id, inputs, dependency_outputs, price).await
        }
        (RobloxInputs::ExperienceActivation(_), RobloxOutputs::ExperienceActivation) => {
            super::create::create(mgr, resource_id, inputs, dependency_outputs, price).await
        }
        (RobloxInputs::ExperienceIcon(_), RobloxOutputs::ExperienceIcon(_)) => {
            super::create::create(mgr, resource_id, inputs, dependency_outputs, price).await
        }
        (RobloxInputs::ExperienceThumbnail(_), RobloxOutputs::ExperienceThumbnail(_)) => {
            super::delete::delete(mgr, resource_id, outputs, dependency_outputs.clone()).await?;
            super::create::create(mgr, resource_id, inputs, dependency_outputs, price).await
        }
        (RobloxInputs::ExperienceThumbnailOrder, RobloxOutputs::ExperienceThumbnailOrder) => {
            super::create::create(mgr, resource_id, inputs, dependency_outputs, price).await
        }
        // TODO: is this correct?
        (RobloxInputs::Place(_), RobloxOutputs::Place(_)) => {
            super::create::create(mgr, resource_id, inputs, dependency_outputs, price).await
        }
        (RobloxInputs::PlaceFile(_), RobloxOutputs::PlaceFile(_)) => {
            super::create::create(mgr, resource_id, inputs, dependency_outputs, price).await
        }
        (RobloxInputs::PlaceConfiguration(_), RobloxOutputs::PlaceConfiguration) => {
            super::create::create(mgr, resource_id, inputs, dependency_outputs, price).await
        }
        (RobloxInputs::SocialLink(inputs), RobloxOutputs::SocialLink(outputs)) => {
            let experience = single_output!(dependency_outputs, RobloxOutputs::Experience);

            api_call!(
                mgr,
                context,
                "update_social_link",
                mgr.roblox_api.update_social_link(
                    experience.asset_id,
                    outputs.asset_id,
                    inputs.title,
                    inputs.url,
                    inputs.link_type,
                )
            );

            Ok(RobloxOutputs::SocialLink(outputs))
        }
        (RobloxInputs::ProductIcon(_), RobloxOutputs::ProductIcon(_)) => {
            super::create::create(mgr, resource_id, inputs, dependency_outputs, price).await
        }
        (RobloxInputs::Product(inputs), RobloxOutputs::Product(outputs)) => {
            let experience = single_output!(dependency_outputs, RobloxOutputs::Experience);

            api_call!(
                mgr,
                context,
                "update_developer_product",
                mgr.roblox_api.update_developer_product(
                    experience.asset_id,
                    outputs.asset_id,
                    inputs.name,
                    inputs.price,
                    inputs.description,
                )
            );

            Ok(RobloxOutputs::Product(outputs))
        }
        (RobloxInputs::Pass(inputs), RobloxOutputs::Pass(outputs)) => {
            let GetGamePassResponse {
                icon_image_asset_id,
                ..
            } = mgr.wrap_api_result(
                context.clone().with_endpoint("update_game_pass"),
                mgr.roblox_api
                    .update_game_pass(
                        outputs.asset_id,
                        inputs.name,
                        inputs.description,
                        inputs.price,
                        Some(mgr.get_path(inputs.icon_file_path)),
                    )
                    .await,
            )?;

            Ok(RobloxOutputs::Pass(PassOutputs {
                asset_id: outputs.asset_id,
                icon_asset_id: icon_image_asset_id,
            }))
        }
        (RobloxInputs::Badge(inputs), RobloxOutputs::Badge(outputs)) => {
            api_call!(
                mgr,
                context,
                "update_badge",
                mgr.roblox_api.update_badge(
                    outputs.asset_id,
                    inputs.name,
                    inputs.description,
                    inputs.enabled,
                )
            );

            Ok(RobloxOutputs::Badge(outputs))
        }
        (RobloxInputs::BadgeIcon(inputs), RobloxOutputs::BadgeIcon(_)) => {
            let badge = single_output!(dependency_outputs, RobloxOutputs::Badge);

            let UploadImageResponse { target_id } = mgr.wrap_api_result(
                context.clone().with_endpoint("update_badge_icon"),
                mgr.roblox_api
                    .update_badge_icon(badge.asset_id, mgr.get_path(inputs.file_path))
                    .await,
            )?;

            Ok(RobloxOutputs::BadgeIcon(AssetOutputs {
                asset_id: target_id,
            }))
        }
        (RobloxInputs::ImageAsset(_), RobloxOutputs::ImageAsset(_)) => {
            super::create::create(mgr, resource_id, inputs, dependency_outputs, price).await
        }
        (RobloxInputs::AudioAsset(_), RobloxOutputs::AudioAsset(_)) => {
            super::create::create(mgr, resource_id, inputs, dependency_outputs, price).await
        }
        (RobloxInputs::AssetAlias(inputs), RobloxOutputs::AssetAlias(outputs)) => {
            let experience = single_output!(dependency_outputs, RobloxOutputs::Experience);

            let image_asset = optional_output!(dependency_outputs, RobloxOutputs::ImageAsset);
            let audio_asset = optional_output!(dependency_outputs, RobloxOutputs::AudioAsset);
            let asset_id = match (image_asset, audio_asset) {
                (Some(image_asset), None) => image_asset.asset_id,
                (None, Some(audio_asset)) => audio_asset.asset_id,
                _ => panic!("Missing expected output."),
            };

            api_call!(
                mgr,
                context,
                "update_asset_alias",
                mgr.roblox_api.update_asset_alias(
                    experience.asset_id,
                    asset_id,
                    outputs.name,
                    inputs.name.clone(),
                )
            );

            if audio_asset.is_some() {
                api_call!(
                    mgr,
                    context,
                    "grant_asset_permissions",
                    mgr.roblox_api.grant_asset_permissions(
                        asset_id,
                        GrantAssetPermissionsRequestRequest {
                            subject_id: experience.asset_id,
                            subject_type: GrantAssetPermissionRequestSubjectType::Universe,
                            action: GrantAssetPermissionRequestAction::Use,
                        },
                    )
                );
            }

            Ok(RobloxOutputs::AssetAlias(AssetAliasOutputs {
                name: inputs.name,
            }))
        }
        (RobloxInputs::SpatialVoice(inputs), RobloxOutputs::SpatialVoice) => {
            let experience = single_output!(dependency_outputs, RobloxOutputs::Experience);

            api_call!(
                mgr,
                context,
                "update_spatial_voice_settings",
                mgr.roblox_api.update_spatial_voice_settings(
                    experience.asset_id,
                    UpdateSpatialVoiceSettingsRequest {
                        opt_in: inputs.enabled,
                    },
                )
            );

            Ok(RobloxOutputs::SpatialVoice)
        }
        (RobloxInputs::Notification(inputs), RobloxOutputs::Notification(outputs)) => {
            let asset_id = outputs.id.clone();
            api_call!(
                mgr,
                context,
                "update_notification",
                mgr.roblox_api
                    .update_notification(asset_id, inputs.name, inputs.content)
            );

            Ok(RobloxOutputs::Notification(outputs))
        }
        _ => unreachable!(),
    }
}

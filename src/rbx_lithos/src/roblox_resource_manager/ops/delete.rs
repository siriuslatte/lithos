//! `delete` dispatcher.
//!
//! Many Roblox resources cannot be hard-deleted via the public API. The
//! match arms below either archive the resource or rename it to a
//! `zzz_DEPRECATED(...)` marker so the deployment can still drop its
//! reference to it cleanly.

use chrono::Utc;
use rbx_api::{
    experiences::models::ExperienceConfigurationModel, places::models::PlaceConfigurationModel,
    spatial_voice::models::UpdateSpatialVoiceSettingsRequest,
};

use crate::diagnostics::{OperationAction, OperationError};
use crate::resource_graph::{all_outputs, single_output};

use super::super::{outputs::RobloxOutputs, RobloxResourceManager};

macro_rules! api_call {
    ($mgr:expr, $context:expr, $endpoint:expr, $call:expr) => {{
        $mgr.wrap_api_result($context.clone().with_endpoint($endpoint), ($call).await)?
    }};
}

// TODO: Do we need inputs?
pub(in crate::roblox_resource_manager) async fn delete(
    mgr: &RobloxResourceManager,
    resource_id: &str,
    outputs: RobloxOutputs,
    dependency_outputs: Vec<RobloxOutputs>,
) -> Result<(), OperationError> {
    let context = mgr.context_for_outputs(OperationAction::Delete, resource_id, &outputs);

    match outputs {
        RobloxOutputs::Experience(outputs) => {
            let model = ExperienceConfigurationModel {
                is_archived: true,
                ..Default::default()
            };
            api_call!(
                mgr,
                context,
                "configure_experience",
                mgr.roblox_api
                    .configure_experience(outputs.asset_id, &model)
            );
        }
        RobloxOutputs::ExperienceConfiguration => {
            let experience = single_output!(dependency_outputs, RobloxOutputs::Experience);

            let model = ExperienceConfigurationModel::default();
            api_call!(
                mgr,
                context,
                "configure_experience",
                mgr.roblox_api
                    .configure_experience(experience.asset_id, &model)
            );
        }
        RobloxOutputs::ExperienceActivation => {
            let experience = single_output!(dependency_outputs, RobloxOutputs::Experience);

            api_call!(
                mgr,
                context,
                "set_experience_active",
                mgr.roblox_api
                    .set_experience_active(experience.asset_id, false)
            );
        }
        RobloxOutputs::ExperienceIcon(outputs) => {
            let experience = single_output!(dependency_outputs, RobloxOutputs::Experience);

            api_call!(
                mgr,
                context,
                "remove_experience_icon",
                mgr.roblox_api
                    .remove_experience_icon(experience.start_place_id, outputs.asset_id)
            );
        }
        RobloxOutputs::ExperienceThumbnail(outputs) => {
            let experience = single_output!(dependency_outputs, RobloxOutputs::Experience);

            api_call!(
                mgr,
                context,
                "delete_experience_thumbnail",
                mgr.roblox_api
                    .delete_experience_thumbnail(experience.asset_id, outputs.asset_id)
            );
        }
        RobloxOutputs::ExperienceThumbnailOrder => {}
        RobloxOutputs::Place(outputs) => {
            let experience = single_output!(dependency_outputs, RobloxOutputs::Experience);

            if outputs.asset_id != experience.start_place_id {
                api_call!(
                    mgr,
                    context,
                    "remove_place_from_experience",
                    mgr.roblox_api
                        .remove_place_from_experience(experience.asset_id, outputs.asset_id)
                );
            }
        }
        RobloxOutputs::PlaceFile(_) => {}
        RobloxOutputs::PlaceConfiguration => {
            let place = single_output!(dependency_outputs, RobloxOutputs::Place);

            let model = PlaceConfigurationModel::default();
            api_call!(
                mgr,
                context,
                "configure_place",
                mgr.roblox_api.configure_place(place.asset_id, &model)
            );
        }
        RobloxOutputs::SocialLink(outputs) => {
            let experience = single_output!(dependency_outputs, RobloxOutputs::Experience);

            api_call!(
                mgr,
                context,
                "delete_social_link",
                mgr.roblox_api
                    .delete_social_link(experience.asset_id, outputs.asset_id)
            );
        }
        RobloxOutputs::ProductIcon(_) => {}
        RobloxOutputs::Product(outputs) => {
            let experience = single_output!(dependency_outputs, RobloxOutputs::Experience);

            let utc = Utc::now();
            api_call!(
                mgr,
                context,
                "update_developer_product",
                mgr.roblox_api.update_developer_product(
                    experience.asset_id,
                    outputs.asset_id,
                    format!("zzz_DEPRECATED({})", utc.format("%F %T%.f")),
                    0,
                    "".to_owned(),
                )
            );
        }
        RobloxOutputs::Pass(outputs) => {
            let utc = Utc::now();
            api_call!(
                mgr,
                context,
                "update_game_pass",
                mgr.roblox_api.update_game_pass(
                    outputs.asset_id,
                    format!("zzz_DEPRECATED({})", utc.format("%F %T%.f")),
                    "".to_owned(),
                    None,
                    None,
                )
            );
        }
        RobloxOutputs::Badge(outputs) => {
            let utc = Utc::now();
            api_call!(
                mgr,
                context,
                "update_badge",
                mgr.roblox_api.update_badge(
                    outputs.asset_id,
                    format!("zzz_DEPRECATED({})", utc.format("%F %T%.f")),
                    "".to_owned(),
                    false,
                )
            );
        }
        RobloxOutputs::BadgeIcon(_) => {}
        RobloxOutputs::ImageAsset(outputs) => {
            // TODO: Can we make this not optional and just not import the image asset? Maybe?
            if let Some(decal_asset_id) = outputs.decal_asset_id {
                api_call!(
                    mgr,
                    context,
                    "archive_asset",
                    mgr.roblox_api.archive_asset(decal_asset_id)
                );
            }
            // TODO: if no decal ID is available use Open Cloud API to archive. rbx_cloud currently doesn't support this API
        }
        RobloxOutputs::AudioAsset(outputs) => {
            api_call!(
                mgr,
                context,
                "archive_asset",
                mgr.roblox_api.archive_asset(outputs.asset_id)
            );
        }
        RobloxOutputs::AssetAlias(outputs) => {
            let experience = single_output!(dependency_outputs, RobloxOutputs::Experience);

            api_call!(
                mgr,
                context,
                "delete_asset_alias",
                mgr.roblox_api
                    .delete_asset_alias(experience.asset_id, outputs.name)
            );
        }
        RobloxOutputs::SpatialVoice => {
            let experience = single_output!(dependency_outputs, RobloxOutputs::Experience);

            api_call!(
                mgr,
                context,
                "update_spatial_voice_settings",
                mgr.roblox_api.update_spatial_voice_settings(
                    experience.asset_id,
                    UpdateSpatialVoiceSettingsRequest { opt_in: false },
                )
            );
        }
        RobloxOutputs::Notification(outputs) => {
            api_call!(
                mgr,
                context,
                "archive_notification",
                mgr.roblox_api.archive_notification(outputs.id)
            );
        }
    }
    Ok(())
}

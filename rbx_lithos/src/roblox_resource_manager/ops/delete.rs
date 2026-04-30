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

use crate::resource_graph::{all_outputs, single_output};

use super::super::{outputs::RobloxOutputs, RobloxResourceManager};

// TODO: Do we need inputs?
pub(in crate::roblox_resource_manager) async fn delete(
    mgr: &RobloxResourceManager,
    outputs: RobloxOutputs,
    dependency_outputs: Vec<RobloxOutputs>,
) -> Result<(), String> {
    match outputs {
        RobloxOutputs::Experience(outputs) => {
            let model = ExperienceConfigurationModel {
                is_archived: true,
                ..Default::default()
            };
            mgr.roblox_api
                .configure_experience(outputs.asset_id, &model)
                .await?;
        }
        RobloxOutputs::ExperienceConfiguration => {
            let experience = single_output!(dependency_outputs, RobloxOutputs::Experience);

            let model = ExperienceConfigurationModel::default();
            mgr.roblox_api
                .configure_experience(experience.asset_id, &model)
                .await?;
        }
        RobloxOutputs::ExperienceActivation => {
            let experience = single_output!(dependency_outputs, RobloxOutputs::Experience);

            mgr.roblox_api
                .set_experience_active(experience.asset_id, false)
                .await?;
        }
        RobloxOutputs::ExperienceIcon(outputs) => {
            let experience = single_output!(dependency_outputs, RobloxOutputs::Experience);

            mgr.roblox_api
                .remove_experience_icon(experience.start_place_id, outputs.asset_id)
                .await?;
        }
        RobloxOutputs::ExperienceThumbnail(outputs) => {
            let experience = single_output!(dependency_outputs, RobloxOutputs::Experience);

            mgr.roblox_api
                .delete_experience_thumbnail(experience.asset_id, outputs.asset_id)
                .await?;
        }
        RobloxOutputs::ExperienceThumbnailOrder => {}
        RobloxOutputs::Place(outputs) => {
            let experience = single_output!(dependency_outputs, RobloxOutputs::Experience);

            if outputs.asset_id != experience.start_place_id {
                mgr.roblox_api
                    .remove_place_from_experience(experience.asset_id, outputs.asset_id)
                    .await?;
            }
        }
        RobloxOutputs::PlaceFile(_) => {}
        RobloxOutputs::PlaceConfiguration => {
            let place = single_output!(dependency_outputs, RobloxOutputs::Place);

            let model = PlaceConfigurationModel::default();
            mgr.roblox_api
                .configure_place(place.asset_id, &model)
                .await?;
        }
        RobloxOutputs::SocialLink(outputs) => {
            let experience = single_output!(dependency_outputs, RobloxOutputs::Experience);

            mgr.roblox_api
                .delete_social_link(experience.asset_id, outputs.asset_id)
                .await?;
        }
        RobloxOutputs::ProductIcon(_) => {}
        RobloxOutputs::Product(outputs) => {
            let experience = single_output!(dependency_outputs, RobloxOutputs::Experience);

            let utc = Utc::now();
            mgr.roblox_api
                .update_developer_product(
                    experience.asset_id,
                    outputs.asset_id,
                    format!("zzz_DEPRECATED({})", utc.format("%F %T%.f")),
                    0,
                    "".to_owned(),
                )
                .await?;
        }
        RobloxOutputs::Pass(outputs) => {
            let utc = Utc::now();
            mgr.roblox_api
                .update_game_pass(
                    outputs.asset_id,
                    format!("zzz_DEPRECATED({})", utc.format("%F %T%.f")),
                    "".to_owned(),
                    None,
                    None,
                )
                .await?;
        }
        RobloxOutputs::Badge(outputs) => {
            let utc = Utc::now();
            mgr.roblox_api
                .update_badge(
                    outputs.asset_id,
                    format!("zzz_DEPRECATED({})", utc.format("%F %T%.f")),
                    "".to_owned(),
                    false,
                )
                .await?;
        }
        RobloxOutputs::BadgeIcon(_) => {}
        RobloxOutputs::ImageAsset(outputs) => {
            // TODO: Can we make this not optional and just not import the image asset? Maybe?
            if let Some(decal_asset_id) = outputs.decal_asset_id {
                mgr.roblox_api.archive_asset(decal_asset_id).await?;
            }
            // TODO: if no decal ID is available use Open Cloud API to archive. rbx_cloud currently doesn't support this API
        }
        RobloxOutputs::AudioAsset(outputs) => {
            mgr.roblox_api.archive_asset(outputs.asset_id).await?;
        }
        RobloxOutputs::AssetAlias(outputs) => {
            let experience = single_output!(dependency_outputs, RobloxOutputs::Experience);

            mgr.roblox_api
                .delete_asset_alias(experience.asset_id, outputs.name)
                .await?;
        }
        RobloxOutputs::SpatialVoice => {
            let experience = single_output!(dependency_outputs, RobloxOutputs::Experience);

            mgr.roblox_api
                .update_spatial_voice_settings(
                    experience.asset_id,
                    UpdateSpatialVoiceSettingsRequest { opt_in: false },
                )
                .await?;
        }
        RobloxOutputs::Notification(outputs) => {
            mgr.roblox_api.archive_notification(outputs.id).await?;
        }
    }
    Ok(())
}

//! `create` dispatcher.
//!
//! One match arm per [`RobloxInputs`] variant. Each arm performs the Roblox
//! API calls required to materialize that resource and returns the matching
//! [`RobloxOutputs`].

use chrono::{DateTime, Duration, Utc};
use rbx_api::{
    asset_permissions::models::{
        GrantAssetPermissionRequestAction, GrantAssetPermissionRequestSubjectType,
        GrantAssetPermissionsRequestRequest,
    },
    assets::models::{CreateAssetQuota, CreateAudioAssetResponse, Creator, QuotaDuration},
    badges::models::CreateBadgeResponse,
    developer_products::models::{
        CreateDeveloperProductIconResponse, CreateDeveloperProductResponse,
        GetDeveloperProductResponse,
    },
    experiences::models::CreateExperienceResponse,
    game_passes::models::{CreateGamePassResponse, GetGamePassResponse},
    models::{AssetTypeId, UploadImageResponse},
    notifications::models::CreateNotificationResponse,
    social_links::models::CreateSocialLinkResponse,
    spatial_voice::models::UpdateSpatialVoiceSettingsRequest,
};
use rbxcloud::rbx::{
    types::{PlaceId, UniverseId},
    v1::PublishVersionType,
};
use yansi::Paint;

use crate::diagnostics::{OperationAction, OperationError};
use crate::resource_graph::{all_outputs, optional_output, single_output};

use super::super::{
    inputs::RobloxInputs,
    outputs::{
        AssetAliasOutputs, AssetOutputs, AssetWithInitialIconOutputs, ExperienceOutputs,
        ImageAssetOutputs, NotificationOutputs, PassOutputs, PlaceFileOutputs, ProductOutputs,
        RobloxOutputs,
    },
    quota::format_quota_reset,
    RobloxResourceManager,
};

macro_rules! api_call {
    ($mgr:expr, $context:expr, $endpoint:expr, $call:expr) => {{
        $mgr.wrap_api_result($context.clone().with_endpoint($endpoint), ($call).await)?
    }};
}

macro_rules! string_call {
    ($mgr:expr, $context:expr, $endpoint:expr, $call:expr) => {{
        $mgr.wrap_message_result(
            $context.clone().with_endpoint($endpoint),
            ($call).await.map_err(|error| error.to_string()),
        )?
    }};
}

pub(in crate::roblox_resource_manager) async fn create(
    mgr: &RobloxResourceManager,
    resource_id: &str,
    inputs: RobloxInputs,
    dependency_outputs: Vec<RobloxOutputs>,
    price: Option<u32>,
) -> Result<RobloxOutputs, OperationError> {
    let context = mgr.context_for_inputs(OperationAction::Create, resource_id, &inputs);

    match inputs {
        RobloxInputs::Experience(inputs) => {
            let CreateExperienceResponse {
                universe_id,
                root_place_id,
            } = api_call!(
                mgr,
                context,
                "create_experience",
                mgr.roblox_api.create_experience(inputs.group_id)
            );

            Ok(RobloxOutputs::Experience(ExperienceOutputs {
                asset_id: universe_id,
                start_place_id: root_place_id,
            }))
        }
        RobloxInputs::ExperienceConfiguration(inputs) => {
            let experience = single_output!(dependency_outputs, RobloxOutputs::Experience);

            api_call!(
                mgr,
                context,
                "configure_experience",
                mgr.roblox_api
                    .configure_experience(experience.asset_id, &inputs)
            );

            Ok(RobloxOutputs::ExperienceConfiguration)
        }
        RobloxInputs::ExperienceActivation(inputs) => {
            let experience = single_output!(dependency_outputs, RobloxOutputs::Experience);

            api_call!(
                mgr,
                context,
                "set_experience_active",
                mgr.roblox_api
                    .set_experience_active(experience.asset_id, inputs.is_active)
            );

            Ok(RobloxOutputs::ExperienceActivation)
        }
        RobloxInputs::ExperienceIcon(inputs) => {
            let experience = single_output!(dependency_outputs, RobloxOutputs::Experience);

            let UploadImageResponse { target_id } = mgr.wrap_api_result(
                context.clone().with_endpoint("upload_icon"),
                mgr.roblox_api
                    .upload_icon(experience.asset_id, mgr.get_path(inputs.file_path))
                    .await,
            )?;

            Ok(RobloxOutputs::ExperienceIcon(AssetOutputs {
                asset_id: target_id,
            }))
        }
        RobloxInputs::ExperienceThumbnail(inputs) => {
            let experience = single_output!(dependency_outputs, RobloxOutputs::Experience);

            let UploadImageResponse { target_id } = mgr.wrap_api_result(
                context.clone().with_endpoint("upload_thumbnail"),
                mgr.roblox_api
                    .upload_thumbnail(experience.asset_id, mgr.get_path(inputs.file_path))
                    .await,
            )?;

            Ok(RobloxOutputs::ExperienceThumbnail(AssetOutputs {
                asset_id: target_id,
            }))
        }
        RobloxInputs::ExperienceThumbnailOrder => {
            let experience = single_output!(dependency_outputs, RobloxOutputs::Experience);
            let thumbnails = all_outputs!(dependency_outputs, RobloxOutputs::ExperienceThumbnail);

            api_call!(
                mgr,
                context,
                "set_experience_thumbnail_order",
                mgr.roblox_api.set_experience_thumbnail_order(
                    experience.asset_id,
                    &thumbnails.iter().map(|t| t.asset_id).collect::<Vec<_>>(),
                )
            );

            Ok(RobloxOutputs::ExperienceThumbnailOrder)
        }
        RobloxInputs::Place(inputs) => {
            let experience = single_output!(dependency_outputs, RobloxOutputs::Experience);

            let asset_id = if inputs.is_start {
                experience.start_place_id
            } else {
                api_call!(
                    mgr,
                    context,
                    "create_place",
                    mgr.roblox_api.create_place(experience.asset_id)
                )
                .place_id
            };

            Ok(RobloxOutputs::Place(AssetOutputs { asset_id }))
        }
        RobloxInputs::PlaceFile(inputs) => {
            let place = single_output!(dependency_outputs, RobloxOutputs::Place);
            let experience = single_output!(dependency_outputs, RobloxOutputs::Experience);

            if let Some(roblox_cloud) = &mgr.roblox_cloud {
                let response = string_call!(
                    mgr,
                    context,
                    "publish_place_version",
                    roblox_cloud
                        .experience(UniverseId(experience.asset_id), PlaceId(place.asset_id))
                        .publish(
                            &mgr.get_path(inputs.file_path)
                                .into_os_string()
                                .into_string()
                                .unwrap(),
                            PublishVersionType::Published,
                        )
                );

                Ok(RobloxOutputs::PlaceFile(PlaceFileOutputs {
                    version: response.version_number,
                }))
            } else {
                Err(mgr.operation_error(
                    context,
                    "Place uploads require Open Cloud authentication. Find out more here: https://mantledeploy.vercel.app/docs/authentication#roblox-open-cloud-api-key",
                ))
            }
        }
        RobloxInputs::PlaceConfiguration(inputs) => {
            let place = single_output!(dependency_outputs, RobloxOutputs::Place);

            api_call!(
                mgr,
                context,
                "configure_place",
                mgr.roblox_api.configure_place(place.asset_id, &inputs)
            );

            Ok(RobloxOutputs::PlaceConfiguration)
        }
        RobloxInputs::SocialLink(inputs) => {
            let experience = single_output!(dependency_outputs, RobloxOutputs::Experience);

            let CreateSocialLinkResponse { id } = mgr.wrap_api_result(
                context.clone().with_endpoint("create_social_link"),
                mgr.roblox_api
                    .create_social_link(
                        experience.asset_id,
                        inputs.title,
                        inputs.url,
                        inputs.link_type,
                    )
                    .await,
            )?;

            Ok(RobloxOutputs::SocialLink(AssetOutputs { asset_id: id }))
        }
        RobloxInputs::ProductIcon(inputs) => {
            let product = single_output!(dependency_outputs, RobloxOutputs::Product);

            let CreateDeveloperProductIconResponse { image_asset_id } = mgr.wrap_api_result(
                context
                    .clone()
                    .with_endpoint("create_developer_product_icon"),
                mgr.roblox_api
                    .create_developer_product_icon(product.asset_id, mgr.get_path(inputs.file_path))
                    .await,
            )?;

            Ok(RobloxOutputs::ProductIcon(AssetOutputs {
                asset_id: image_asset_id,
            }))
        }
        RobloxInputs::Product(inputs) => {
            let experience = single_output!(dependency_outputs, RobloxOutputs::Experience);

            let CreateDeveloperProductResponse { id } = mgr.wrap_api_result(
                context.clone().with_endpoint("create_developer_product"),
                mgr.roblox_api
                    .create_developer_product(
                        experience.asset_id,
                        inputs.name,
                        inputs.price,
                        inputs.description,
                    )
                    .await,
            )?;

            let GetDeveloperProductResponse { id: product_id } = mgr.wrap_api_result(
                context.clone().with_endpoint("get_developer_product"),
                mgr.roblox_api.get_developer_product(id).await,
            )?;

            Ok(RobloxOutputs::Product(ProductOutputs {
                asset_id: product_id,
                product_id: id,
            }))
        }
        RobloxInputs::Pass(inputs) => {
            let experience = single_output!(dependency_outputs, RobloxOutputs::Experience);

            let CreateGamePassResponse { game_pass_id } = mgr.wrap_api_result(
                context.clone().with_endpoint("create_game_pass"),
                mgr.roblox_api
                    .create_game_pass(
                        experience.asset_id,
                        inputs.name.clone(),
                        inputs.description.clone(),
                        mgr.get_path(inputs.icon_file_path),
                    )
                    .await,
            )?;
            let GetGamePassResponse {
                icon_image_asset_id,
                ..
            } = mgr.wrap_api_result(
                context.clone().with_endpoint("update_game_pass"),
                mgr.roblox_api
                    .update_game_pass(
                        game_pass_id,
                        inputs.name,
                        inputs.description,
                        inputs.price,
                        None,
                    )
                    .await,
            )?;

            Ok(RobloxOutputs::Pass(PassOutputs {
                asset_id: game_pass_id,
                icon_asset_id: icon_image_asset_id,
            }))
        }
        RobloxInputs::Badge(inputs) => {
            let experience = single_output!(dependency_outputs, RobloxOutputs::Experience);

            let CreateBadgeResponse { id, icon_image_id } = mgr.wrap_api_result(
                context.clone().with_endpoint("create_badge"),
                mgr.roblox_api
                    .create_badge(
                        experience.asset_id,
                        inputs.name,
                        inputs.description,
                        mgr.get_path(inputs.icon_file_path),
                        mgr.payment_source.clone(),
                        price.unwrap_or(0),
                    )
                    .await,
            )?;

            Ok(RobloxOutputs::Badge(AssetWithInitialIconOutputs {
                asset_id: id,
                initial_icon_asset_id: icon_image_id,
            }))
        }
        RobloxInputs::BadgeIcon(_) => {
            let badge = single_output!(dependency_outputs, RobloxOutputs::Badge);

            Ok(RobloxOutputs::BadgeIcon(AssetOutputs {
                asset_id: badge.initial_icon_asset_id,
            }))
        }
        RobloxInputs::ImageAsset(inputs) => {
            let creator = match inputs.group_id {
                Some(group_id) => Creator::GroupId(group_id.to_string()),
                None => Creator::UserId(mgr.user.id.to_string()),
            };
            let asset_id = mgr.wrap_api_result(
                context.clone().with_endpoint("create_image_asset"),
                mgr.roblox_api
                    .create_image_asset(mgr.get_path(&inputs.file_path), creator)
                    .await,
            )?;
            Ok(RobloxOutputs::ImageAsset(ImageAssetOutputs {
                asset_id,
                // TODO: This breaks archiving assets.
                decal_asset_id: None,
            }))
        }
        RobloxInputs::AudioAsset(inputs) => {
            let CreateAssetQuota {
                usage,
                capacity,
                expiration_time,
                duration,
            } = mgr.wrap_api_result(
                context.clone().with_endpoint("get_create_asset_quota"),
                mgr.roblox_api
                    .get_create_asset_quota(AssetTypeId::Audio)
                    .await,
            )?;

            let quota_reset = format_quota_reset(match expiration_time {
                Some(ref x) => DateTime::parse_from_rfc3339(x)
                    .map_err(|e| {
                        mgr.operation_error(
                            context.clone().with_endpoint("get_create_asset_quota"),
                            format!("Unable to parse expiration_time: {}", e),
                        )
                    })?
                    .with_timezone(&Utc),
                None => {
                    Utc::now()
                        + match duration {
                            // TODO: Learn how Roblox computes a "Month" to ensure this is an accurate estimate
                            QuotaDuration::Month => Duration::days(30),
                        }
                }
            });

            if usage < capacity {
                logger::log("");
                logger::log(Paint::yellow(format!(
                    "You will have {} audio upload(s) remaining in the current period after creation. Your quota will reset in {}.",
                    capacity - usage - 1,
                    quota_reset
                )));

                let CreateAudioAssetResponse { id } = mgr.wrap_api_result(
                    context.clone().with_endpoint("create_audio_asset"),
                    mgr.roblox_api
                        .create_audio_asset(
                            mgr.get_path(inputs.file_path),
                            inputs.group_id,
                            mgr.payment_source.clone(),
                        )
                        .await,
                )?;

                Ok(RobloxOutputs::AudioAsset(AssetOutputs { asset_id: id }))
            } else {
                Err(mgr.operation_error(
                    context,
                    format!(
                        "You have reached your audio upload quota. Your quota will reset in {}.",
                        quota_reset
                    ),
                ))
            }
        }
        RobloxInputs::AssetAlias(inputs) => {
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
                "create_asset_alias",
                mgr.roblox_api.create_asset_alias(
                    experience.asset_id,
                    asset_id,
                    inputs.name.clone()
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
        RobloxInputs::SpatialVoice(inputs) => {
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
        RobloxInputs::Notification(inputs) => {
            let experience = single_output!(dependency_outputs, RobloxOutputs::Experience);

            let CreateNotificationResponse { id } = mgr.wrap_api_result(
                context.clone().with_endpoint("create_notification"),
                mgr.roblox_api
                    .create_notification(experience.asset_id, inputs.name, inputs.content)
                    .await,
            )?;

            Ok(RobloxOutputs::Notification(NotificationOutputs { id }))
        }
    }
}

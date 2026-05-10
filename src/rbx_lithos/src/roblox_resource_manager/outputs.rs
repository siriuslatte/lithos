//! Output data types for [`RobloxResource`](super::RobloxResource).
//!
//! Pure data describing the realized state of a Roblox resource after it
//! has been created on the platform.

use rbx_api::models::AssetId;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExperienceOutputs {
    pub asset_id: AssetId,
    pub start_place_id: AssetId,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AssetOutputs {
    pub asset_id: AssetId,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NotificationOutputs {
    pub id: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlaceFileOutputs {
    pub version: u64,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProductOutputs {
    pub asset_id: AssetId,
    pub product_id: AssetId,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PassOutputs {
    pub asset_id: AssetId,
    pub icon_asset_id: AssetId,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AssetWithInitialIconOutputs {
    pub asset_id: AssetId,
    pub initial_icon_asset_id: AssetId,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ImageAssetOutputs {
    pub asset_id: AssetId,
    pub decal_asset_id: Option<AssetId>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AssetAliasOutputs {
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum RobloxOutputs {
    Experience(ExperienceOutputs),
    ExperienceConfiguration,
    ExperienceActivation,
    ExperienceIcon(AssetOutputs),
    ExperienceThumbnail(AssetOutputs),
    ExperienceThumbnailOrder,
    Place(AssetOutputs),
    PlaceFile(PlaceFileOutputs),
    PlaceConfiguration,
    SocialLink(AssetOutputs),
    Product(ProductOutputs),
    ProductIcon(AssetOutputs),
    Pass(PassOutputs),
    Badge(AssetWithInitialIconOutputs),
    BadgeIcon(AssetOutputs),
    ImageAsset(ImageAssetOutputs),
    AudioAsset(AssetOutputs),
    AssetAlias(AssetAliasOutputs),
    SpatialVoice,
    Notification(NotificationOutputs),
}

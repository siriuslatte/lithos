//! Input data types for [`RobloxResource`](super::RobloxResource).
//!
//! These are pure data structures describing the desired state of a Roblox
//! resource. They contain no behavior; effectful logic lives in the manager
//! and operations modules.

use rbx_api::{
    experiences::models::ExperienceConfigurationModel, models::AssetId,
    places::models::PlaceConfigurationModel, social_links::models::SocialLinkType,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExperienceInputs {
    pub group_id: Option<AssetId>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExperienceActivationInputs {
    pub is_active: bool,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileInputs {
    pub file_path: String,
    pub file_hash: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlaceInputs {
    pub is_start: bool,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SocialLinkInputs {
    pub title: String,
    pub url: String,
    pub link_type: SocialLinkType,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProductInputs {
    pub name: String,
    pub description: String,
    pub price: u32,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PassInputs {
    pub name: String,
    pub description: String,
    pub price: Option<u32>,
    pub icon_file_path: String,
    pub icon_file_hash: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BadgeInputs {
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub icon_file_path: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileWithGroupIdInputs {
    pub file_path: String,
    pub file_hash: String,
    pub group_id: Option<AssetId>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AssetAliasInputs {
    pub name: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SpatialVoiceInputs {
    pub enabled: bool,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NotificationInputs {
    pub name: String,
    pub content: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::large_enum_variant)]
pub enum RobloxInputs {
    Experience(ExperienceInputs),
    ExperienceConfiguration(ExperienceConfigurationModel),
    ExperienceActivation(ExperienceActivationInputs),
    ExperienceIcon(FileInputs),
    ExperienceThumbnail(FileInputs),
    ExperienceThumbnailOrder,
    Place(PlaceInputs),
    PlaceFile(FileInputs),
    PlaceConfiguration(PlaceConfigurationModel),
    SocialLink(SocialLinkInputs),
    Product(ProductInputs),
    ProductIcon(FileInputs),
    Pass(PassInputs),
    Badge(BadgeInputs),
    BadgeIcon(FileInputs),
    ImageAsset(FileWithGroupIdInputs),
    AudioAsset(FileWithGroupIdInputs),
    AssetAlias(AssetAliasInputs),
    SpatialVoice(SpatialVoiceInputs),
    Notification(NotificationInputs),
}

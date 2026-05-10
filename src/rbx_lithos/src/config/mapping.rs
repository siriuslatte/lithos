//! Pure mapping from user-facing config types to Roblox API model types.
//!
//! These `From` impls translate the validated config tree into the request
//! shape expected by `rbx_api`. They are deliberately separated from the
//! type definitions in the parent [`config`](super) module to keep that
//! module focused on the data shape and YAML schema, and from the
//! [`loading`](super::loading) module which owns filesystem effects.

use rbx_api::{
    experiences::models::{
        ExperienceAnimationType, ExperienceAvatarType, ExperienceCollisionType,
        ExperienceConfigurationModel, ExperienceGenre, ExperiencePlayableDevice,
    },
    models::{AssetTypeId, SocialSlotType},
    places::models::PlaceConfigurationModel,
};

use super::{
    AnimationTypeTargetConfig, AvatarTypeTargetConfig, CollisionTypeTargetConfig,
    ExperienceTargetConfigurationConfig, GenreTargetConfig, PaidAccessTargetConfig,
    PlaceTargetConfigurationConfig, PlayabilityTargetConfig, PlayableDeviceTargetConfig,
    PrivateServersTargetConfig, ServerFillTargetConfig,
};

impl From<&ExperienceTargetConfigurationConfig> for ExperienceConfigurationModel {
    fn from(config: &ExperienceTargetConfigurationConfig) -> Self {
        let mut model = ExperienceConfigurationModel::default();
        if let Some(genre) = &config.genre {
            model.genre = match genre {
                GenreTargetConfig::All => ExperienceGenre::All,
                GenreTargetConfig::Adventure => ExperienceGenre::Adventure,
                GenreTargetConfig::Building => ExperienceGenre::Tutorial,
                GenreTargetConfig::Comedy => ExperienceGenre::Funny,
                GenreTargetConfig::Fighting => ExperienceGenre::Ninja,
                GenreTargetConfig::Fps => ExperienceGenre::Fps,
                GenreTargetConfig::Horror => ExperienceGenre::Scary,
                GenreTargetConfig::Medieval => ExperienceGenre::Fantasy,
                GenreTargetConfig::Military => ExperienceGenre::War,
                GenreTargetConfig::Naval => ExperienceGenre::Pirate,
                GenreTargetConfig::Rpg => ExperienceGenre::Rpg,
                GenreTargetConfig::SciFi => ExperienceGenre::SciFi,
                GenreTargetConfig::Sports => ExperienceGenre::Sports,
                GenreTargetConfig::TownAndCity => ExperienceGenre::TownAndCity,
                GenreTargetConfig::Western => ExperienceGenre::WildWest,
            }
        }
        if let Some(playable_devices) = &config.playable_devices {
            model.playable_devices = playable_devices
                .iter()
                .map(|device| match device {
                    PlayableDeviceTargetConfig::Computer => ExperiencePlayableDevice::Computer,
                    PlayableDeviceTargetConfig::Phone => ExperiencePlayableDevice::Phone,
                    PlayableDeviceTargetConfig::Tablet => ExperiencePlayableDevice::Tablet,
                    PlayableDeviceTargetConfig::Console => ExperiencePlayableDevice::Console,
                    PlayableDeviceTargetConfig::VR => ExperiencePlayableDevice::VR,
                })
                .collect();
        }
        if let Some(playability) = &config.playability {
            model.is_friends_only = match playability {
                PlayabilityTargetConfig::Friends => Some(true),
                PlayabilityTargetConfig::Public => Some(false),
                PlayabilityTargetConfig::Private => None,
            }
        }
        model.is_for_sale = !matches!(config.paid_access, PaidAccessTargetConfig::Disabled);
        model.price = match config.paid_access {
            PaidAccessTargetConfig::Price(price) => Some(price),
            _ => None,
        };
        model.allow_private_servers =
            !matches!(config.private_servers, PrivateServersTargetConfig::Disabled);
        model.private_server_price = match config.private_servers {
            PrivateServersTargetConfig::Free => Some(0),
            PrivateServersTargetConfig::Price(price) => Some(price),
            _ => None,
        };
        if let Some(enable_studio_access_to_apis) = config.enable_studio_access_to_apis {
            model.studio_access_to_apis_allowed = enable_studio_access_to_apis;
        }
        if let Some(allow_third_party_sales) = config.allow_third_party_sales {
            model.permissions.is_third_party_purchase_allowed = allow_third_party_sales;
        }
        if let Some(allow_third_party_teleports) = config.allow_third_party_teleports {
            model.permissions.is_third_party_teleport_allowed = allow_third_party_teleports;
        }
        if let Some(avatar_type) = &config.avatar_type {
            model.universe_avatar_type = match avatar_type {
                AvatarTypeTargetConfig::R6 => ExperienceAvatarType::MorphToR6,
                AvatarTypeTargetConfig::R15 => ExperienceAvatarType::MorphToR15,
                AvatarTypeTargetConfig::PlayerChoice => ExperienceAvatarType::PlayerChoice,
            }
        }
        if let Some(avatar_animation_type) = &config.avatar_animation_type {
            model.universe_animation_type = match avatar_animation_type {
                AnimationTypeTargetConfig::Standard => ExperienceAnimationType::Standard,
                AnimationTypeTargetConfig::PlayerChoice => ExperienceAnimationType::PlayerChoice,
            }
        }
        if let Some(avatar_collision_type) = &config.avatar_collision_type {
            model.universe_collision_type = match avatar_collision_type {
                CollisionTypeTargetConfig::OuterBox => ExperienceCollisionType::OuterBox,
                CollisionTypeTargetConfig::InnerBox => ExperienceCollisionType::InnerBox,
            }
        }
        if let Some(constraints) = &config.avatar_scale_constraints {
            if let Some(height) = constraints.height.and_then(|c| c.min) {
                model.universe_avatar_min_scales.height = height.to_string();
            }
            if let Some(width) = constraints.width.and_then(|c| c.min) {
                model.universe_avatar_min_scales.width = width.to_string();
            }
            if let Some(head) = constraints.head.and_then(|c| c.min) {
                model.universe_avatar_min_scales.head = head.to_string();
            }
            if let Some(body_type) = constraints.body_type.and_then(|c| c.min) {
                model.universe_avatar_min_scales.body_type = body_type.to_string();
            }
            if let Some(proportions) = constraints.proportions.and_then(|c| c.min) {
                model.universe_avatar_min_scales.proportion = proportions.to_string();
            }

            if let Some(height) = constraints.height.and_then(|c| c.max) {
                model.universe_avatar_max_scales.height = height.to_string();
            }
            if let Some(width) = constraints.width.and_then(|c| c.max) {
                model.universe_avatar_max_scales.width = width.to_string();
            }
            if let Some(head) = constraints.head.and_then(|c| c.max) {
                model.universe_avatar_max_scales.head = head.to_string();
            }
            if let Some(body_type) = constraints.body_type.and_then(|c| c.max) {
                model.universe_avatar_max_scales.body_type = body_type.to_string();
            }
            if let Some(proportions) = constraints.proportions.and_then(|c| c.max) {
                model.universe_avatar_max_scales.proportion = proportions.to_string();
            }
        }
        if let Some(avatar_asset_overrides) = &config.avatar_asset_overrides {
            for override_model in model.universe_avatar_asset_overrides.iter_mut() {
                if let Some(override_config) = match override_model.asset_type_id {
                    AssetTypeId::Face => avatar_asset_overrides.face,
                    AssetTypeId::Head => avatar_asset_overrides.head,
                    AssetTypeId::Torso => avatar_asset_overrides.torso,
                    AssetTypeId::LeftArm => avatar_asset_overrides.left_arm,
                    AssetTypeId::RightArm => avatar_asset_overrides.right_arm,
                    AssetTypeId::LeftLeg => avatar_asset_overrides.left_leg,
                    AssetTypeId::RightLeg => avatar_asset_overrides.right_leg,
                    AssetTypeId::TShirt => avatar_asset_overrides.t_shirt,
                    AssetTypeId::Shirt => avatar_asset_overrides.shirt,
                    AssetTypeId::Pants => avatar_asset_overrides.pants,
                    _ => None,
                } {
                    override_model.is_player_choice = false;
                    override_model.asset_id = Some(override_config);
                }
            }
        }
        model
    }
}

impl From<PlaceTargetConfigurationConfig> for PlaceConfigurationModel {
    fn from(config: PlaceTargetConfigurationConfig) -> Self {
        let mut model = PlaceConfigurationModel::default();
        if let Some(name) = config.name {
            model.name = name;
        }
        if let Some(description) = config.description {
            model.description = description;
        }
        if let Some(max_player_count) = config.max_player_count {
            model.max_player_count = max_player_count;
        }
        if let Some(allow_copying) = config.allow_copying {
            model.allow_copying = allow_copying;
        }
        if let Some(server_fill) = config.server_fill {
            model.social_slot_type = match server_fill {
                ServerFillTargetConfig::RobloxOptimized => SocialSlotType::Automatic,
                ServerFillTargetConfig::Maximum => SocialSlotType::Empty,
                ServerFillTargetConfig::ReservedSlots(_) => SocialSlotType::Custom,
            };
            model.custom_social_slots_count = match server_fill {
                ServerFillTargetConfig::ReservedSlots(count) => Some(count),
                _ => None,
            }
        }
        model
    }
}

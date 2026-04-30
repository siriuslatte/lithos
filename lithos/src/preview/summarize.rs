//! Pure summarization helpers for the plan preview.
//!
//! These functions translate raw [`RobloxInputs`] values into:
//!
//! - a short, human-readable resource type label;
//! - a friendly display name for the row;
//! - a field-level summary of what changed between two input values;
//! - a risk classification for destructive operations.
//!
//! Keeping the logic here means the rest of the preview pipeline – plan
//! construction, rendering, and tests – never has to match on
//! `RobloxInputs` variants directly.

use rbx_lithos::roblox_resource_manager::RobloxInputs;

use super::model::RiskLevel;

/// A short, capitalized noun phrase for the resource type, used in the
/// preview's first column.
pub fn resource_type_label(inputs: &RobloxInputs) -> &'static str {
    match inputs {
        RobloxInputs::Experience(_) => "Experience",
        RobloxInputs::ExperienceConfiguration(_) => "Experience Configuration",
        RobloxInputs::ExperienceActivation(_) => "Experience Activation",
        RobloxInputs::ExperienceIcon(_) => "Experience Icon",
        RobloxInputs::ExperienceThumbnail(_) => "Experience Thumbnail",
        RobloxInputs::ExperienceThumbnailOrder => "Experience Thumbnail Order",
        RobloxInputs::Place(_) => "Place",
        RobloxInputs::PlaceFile(_) => "Place File",
        RobloxInputs::PlaceConfiguration(_) => "Place Configuration",
        RobloxInputs::SocialLink(_) => "Social Link",
        RobloxInputs::Product(_) => "Developer Product",
        RobloxInputs::ProductIcon(_) => "Developer Product Icon",
        RobloxInputs::Pass(_) => "Game Pass",
        RobloxInputs::Badge(_) => "Badge",
        RobloxInputs::BadgeIcon(_) => "Badge Icon",
        RobloxInputs::ImageAsset(_) => "Image Asset",
        RobloxInputs::AudioAsset(_) => "Audio Asset",
        RobloxInputs::AssetAlias(_) => "Asset Alias",
        RobloxInputs::SpatialVoice(_) => "Spatial Voice",
        RobloxInputs::Notification(_) => "Notification",
    }
}

/// Returns a friendly display name for the row, falling back to the resource
/// id when no user-facing name exists for that input variant.
pub fn human_label(inputs: &RobloxInputs, resource_id: &str) -> String {
    match inputs {
        RobloxInputs::Product(p) => p.name.clone(),
        RobloxInputs::Pass(p) => p.name.clone(),
        RobloxInputs::Badge(b) => b.name.clone(),
        RobloxInputs::Notification(n) => n.name.clone(),
        RobloxInputs::AssetAlias(a) => a.name.clone(),
        RobloxInputs::SocialLink(s) => s.title.clone(),
        _ => resource_id.to_owned(),
    }
}

/// Risk classification for deletions. Resources that cost the user money
/// (developer products, game passes) or affect live players (badges,
/// notifications) are flagged as destructive so the renderer can warn.
pub fn risk_for_delete(inputs: &RobloxInputs) -> RiskLevel {
    match inputs {
        RobloxInputs::Product(_)
        | RobloxInputs::Pass(_)
        | RobloxInputs::Badge(_)
        | RobloxInputs::Notification(_)
        | RobloxInputs::Experience(_)
        | RobloxInputs::Place(_) => RiskLevel::Destructive,
        _ => RiskLevel::Caution,
    }
}

/// Produces a list of "key: prev -> next" lines for the change. Returns an
/// empty list if no field-level summary is available for that variant or if
/// nothing relevant changed.
pub fn summarize_change(prev: &RobloxInputs, next: &RobloxInputs) -> Vec<String> {
    let mut lines = Vec::new();
    match (prev, next) {
        (RobloxInputs::Product(a), RobloxInputs::Product(b)) => {
            push_str_diff(&mut lines, "name", &a.name, &b.name);
            push_str_diff(&mut lines, "description", &a.description, &b.description);
            push_num_diff(&mut lines, "price", a.price, b.price);
        }
        (RobloxInputs::Pass(a), RobloxInputs::Pass(b)) => {
            push_str_diff(&mut lines, "name", &a.name, &b.name);
            push_str_diff(&mut lines, "description", &a.description, &b.description);
            push_opt_num_diff(&mut lines, "price", a.price, b.price);
            push_str_diff(&mut lines, "icon", &a.icon_file_hash, &b.icon_file_hash);
        }
        (RobloxInputs::Badge(a), RobloxInputs::Badge(b)) => {
            push_str_diff(&mut lines, "name", &a.name, &b.name);
            push_str_diff(&mut lines, "description", &a.description, &b.description);
            push_bool_diff(&mut lines, "enabled", a.enabled, b.enabled);
            push_str_diff(
                &mut lines,
                "icon path",
                &a.icon_file_path,
                &b.icon_file_path,
            );
        }
        (RobloxInputs::Notification(a), RobloxInputs::Notification(b)) => {
            push_str_diff(&mut lines, "name", &a.name, &b.name);
            push_str_diff(&mut lines, "content", &a.content, &b.content);
        }
        (RobloxInputs::SocialLink(a), RobloxInputs::SocialLink(b)) => {
            push_str_diff(&mut lines, "title", &a.title, &b.title);
            push_str_diff(&mut lines, "url", &a.url, &b.url);
        }
        (RobloxInputs::ExperienceActivation(a), RobloxInputs::ExperienceActivation(b)) => {
            push_bool_diff(&mut lines, "active", a.is_active, b.is_active);
        }
        (RobloxInputs::SpatialVoice(a), RobloxInputs::SpatialVoice(b)) => {
            push_bool_diff(&mut lines, "enabled", a.enabled, b.enabled);
        }
        (RobloxInputs::PlaceFile(a), RobloxInputs::PlaceFile(b))
        | (RobloxInputs::ExperienceIcon(a), RobloxInputs::ExperienceIcon(b))
        | (RobloxInputs::ExperienceThumbnail(a), RobloxInputs::ExperienceThumbnail(b))
        | (RobloxInputs::ProductIcon(a), RobloxInputs::ProductIcon(b))
        | (RobloxInputs::BadgeIcon(a), RobloxInputs::BadgeIcon(b)) => {
            if a.file_hash != b.file_hash {
                lines.push(format!(
                    "file contents changed ({})",
                    short_hash_diff(&a.file_hash, &b.file_hash)
                ));
            }
            if a.file_path != b.file_path {
                lines.push(format!("path: {} -> {}", a.file_path, b.file_path));
            }
        }
        (RobloxInputs::AssetAlias(a), RobloxInputs::AssetAlias(b)) => {
            push_str_diff(&mut lines, "name", &a.name, &b.name);
        }
        (RobloxInputs::Place(a), RobloxInputs::Place(b)) => {
            push_bool_diff(&mut lines, "is start place", a.is_start, b.is_start);
        }
        // For configuration models and other variants we don't try to mirror
        // every field; the deploy step will still apply the full update, and
        // the renderer falls back to a generic "configuration changed" line.
        _ => {}
    }
    if lines.is_empty() {
        lines.push("configuration updated".to_owned());
    }
    lines
}

fn push_str_diff(out: &mut Vec<String>, key: &str, a: &str, b: &str) {
    if a != b {
        out.push(format!("{}: {} -> {}", key, truncate(a), truncate(b)));
    }
}

fn push_num_diff(out: &mut Vec<String>, key: &str, a: u32, b: u32) {
    if a != b {
        out.push(format!("{}: {} -> {}", key, a, b));
    }
}

fn push_opt_num_diff(out: &mut Vec<String>, key: &str, a: Option<u32>, b: Option<u32>) {
    if a != b {
        let fmt = |v: Option<u32>| match v {
            Some(n) => n.to_string(),
            None => "(unset)".to_owned(),
        };
        out.push(format!("{}: {} -> {}", key, fmt(a), fmt(b)));
    }
}

fn push_bool_diff(out: &mut Vec<String>, key: &str, a: bool, b: bool) {
    if a != b {
        out.push(format!("{}: {} -> {}", key, a, b));
    }
}

fn truncate(s: &str) -> String {
    const MAX: usize = 60;
    if s.chars().count() <= MAX {
        s.replace('\n', " ")
    } else {
        let prefix: String = s.chars().take(MAX).collect();
        format!("{}…", prefix.replace('\n', " "))
    }
}

fn short_hash_diff(a: &str, b: &str) -> String {
    let head = |s: &str| s.chars().take(8).collect::<String>();
    format!("{} -> {}", head(a), head(b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rbx_lithos::roblox_resource_manager::ProductInputs;

    #[test]
    fn product_price_change_is_summarized() {
        let prev = RobloxInputs::Product(ProductInputs {
            name: "Coins".to_owned(),
            description: "Buy coins".to_owned(),
            price: 100,
        });
        let next = RobloxInputs::Product(ProductInputs {
            name: "Coins".to_owned(),
            description: "Buy coins".to_owned(),
            price: 150,
        });
        let lines = summarize_change(&prev, &next);
        assert!(
            lines.iter().any(|l| l.contains("price: 100 -> 150")),
            "got {:?}",
            lines
        );
    }

    #[test]
    fn delete_developer_product_is_destructive() {
        let inputs = RobloxInputs::Product(ProductInputs {
            name: "x".to_owned(),
            description: String::new(),
            price: 1,
        });
        assert_eq!(risk_for_delete(&inputs), RiskLevel::Destructive);
    }
}

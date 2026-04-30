//! Pure helpers for formatting Roblox API quota messages.

use chrono::{DateTime, Utc};

/// Format the human-readable time remaining until the given quota reset.
///
/// Pure function: depends only on `reset` and the current UTC clock.
pub(crate) fn format_quota_reset(reset: DateTime<Utc>) -> String {
    let now = Utc::now();
    let duration = reset.signed_duration_since(now);

    let mut parts = Vec::<String>::new();
    if duration.num_days() > 0 {
        parts.push(format!("{}d", duration.num_days()));
    }
    if duration.num_hours() > 0 {
        parts.push(format!(
            "{}h",
            duration.num_hours() - duration.num_days() * 24
        ));
    }
    if duration.num_minutes() > 0 {
        parts.push(format!(
            "{}m",
            duration.num_minutes() - duration.num_hours() * 60
        ));
    }
    if duration.num_seconds() > 0 {
        parts.push(format!(
            "{}s",
            duration.num_seconds() - duration.num_minutes() * 60
        ));
    } else {
        parts.push(format!(
            "{}ms",
            duration.num_milliseconds() - duration.num_seconds() * 1000
        ));
    }

    parts.join(" ")
}

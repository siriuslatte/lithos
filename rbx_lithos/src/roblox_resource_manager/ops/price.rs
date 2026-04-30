//! Pricing dispatcher for resource creation.

use chrono::{Duration, Timelike, Utc};
use yansi::Paint;

use crate::resource_graph::{all_outputs, single_output};

use super::super::{
    inputs::RobloxInputs, outputs::RobloxOutputs, quota::format_quota_reset, RobloxResourceManager,
};

pub(in crate::roblox_resource_manager) async fn get_create_price(
    mgr: &RobloxResourceManager,
    inputs: RobloxInputs,
    dependency_outputs: Vec<RobloxOutputs>,
) -> Result<Option<u32>, String> {
    match inputs {
        RobloxInputs::Badge(_) => {
            let experience = single_output!(dependency_outputs, RobloxOutputs::Experience);
            let free_quota = mgr
                .roblox_api
                .get_create_badge_free_quota(experience.asset_id)
                .await?;

            let quota_reset = format_quota_reset(
                (Utc::now() + Duration::days(1))
                    .with_hour(0)
                    .unwrap()
                    .with_minute(0)
                    .unwrap()
                    .with_second(0)
                    .unwrap()
                    .with_nanosecond(0)
                    .unwrap(),
            );

            if free_quota > 0 {
                logger::log("");
                logger::log(Paint::yellow(
                    format!("You will have {} free badge(s) remaining in the current period after creation. Your quota will reset in {}.", free_quota - 1, quota_reset),
                ));
                Ok(None)
            } else {
                logger::log("");
                logger::log(Paint::yellow(
                    format!("You have no free badges remaining in the current period. Your quota will reset in {}.", quota_reset),
                ));

                Ok(Some(100))
            }
        }
        _ => Ok(None),
    }
}

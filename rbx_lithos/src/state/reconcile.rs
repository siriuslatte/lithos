//! Live reconciliation for persisted resource state.
//!
//! Persisted state can drift from real Roblox resources whenever assets are
//! changed or deleted outside of Lithos. This module verifies the existence
//! (and minimal identity) of persisted resources against the live Roblox API
//! before the diff/apply phase, and produces:
//!
//! - a [`ReconciliationReport`] classifying every persisted resource, and
//! - a reconciled previous-state graph that is safe to feed into the existing
//!   diff/apply pipeline (resources that have been deleted out-of-band are
//!   removed so they will be re-created rather than updated).
//!
//! The module is intentionally split into:
//! - pure data types and helpers ([`VerificationStatus`],
//!   [`ReconciliationReport`], [`reconcile_graph_with_statuses`]) that have
//!   no side effects and are easy to unit test, and
//! - an effectful adapter ([`RobloxLiveStateVerifier`]) that performs the
//!   actual API calls.

use std::collections::BTreeMap;

use async_trait::async_trait;
use rbx_api::{errors::RobloxApiError, RobloxApi};

use crate::{
    resource_graph::{Resource, ResourceGraph, ResourceId},
    roblox_resource_manager::{RobloxInputs, RobloxOutputs, RobloxResource},
};

/// Outcome of verifying a single persisted resource against live Roblox state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationStatus {
    /// The resource exists on Roblox and matches expected identity.
    Verified,
    /// The resource is confirmed deleted on Roblox (e.g. 404 from the
    /// authoritative endpoint).
    Missing,
    /// Verification was not attempted because the resource type has no
    /// supported live check, or because it has no outputs to verify.
    Skipped(String),
    /// Verification was attempted but produced an inconclusive result
    /// (e.g. permission denied, transport error). The resource is left
    /// untouched.
    Unknown(String),
}

impl VerificationStatus {
    pub fn label(&self) -> &'static str {
        match self {
            VerificationStatus::Verified => "verified",
            VerificationStatus::Missing => "missing",
            VerificationStatus::Skipped(_) => "skipped",
            VerificationStatus::Unknown(_) => "unknown",
        }
    }
}

/// Aggregated reconciliation results, indexed by resource id.
#[derive(Debug, Clone)]
pub struct ReconciliationReport {
    pub entries: BTreeMap<ResourceId, VerificationStatus>,
}

impl ReconciliationReport {
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    pub fn missing(&self) -> Vec<&ResourceId> {
        self.entries
            .iter()
            .filter_map(|(id, s)| matches!(s, VerificationStatus::Missing).then_some(id))
            .collect()
    }

    pub fn unknown(&self) -> Vec<(&ResourceId, &str)> {
        self.entries
            .iter()
            .filter_map(|(id, s)| match s {
                VerificationStatus::Unknown(reason) => Some((id, reason.as_str())),
                _ => None,
            })
            .collect()
    }

    pub fn has_drift(&self) -> bool {
        self.entries
            .values()
            .any(|s| matches!(s, VerificationStatus::Missing))
    }

    pub fn counts(&self) -> ReconciliationCounts {
        let mut c = ReconciliationCounts::default();
        for s in self.entries.values() {
            match s {
                VerificationStatus::Verified => c.verified += 1,
                VerificationStatus::Missing => c.missing += 1,
                VerificationStatus::Skipped(_) => c.skipped += 1,
                VerificationStatus::Unknown(_) => c.unknown += 1,
            }
        }
        c
    }
}

impl Default for ReconciliationReport {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ReconciliationCounts {
    pub verified: u32,
    pub missing: u32,
    pub skipped: u32,
    pub unknown: u32,
}

/// Effectful boundary used to verify a single persisted resource.
///
/// Production code uses [`RobloxLiveStateVerifier`]; tests can supply an
/// in-memory implementation.
#[async_trait]
pub trait LiveStateVerifier: Send + Sync {
    async fn verify(&self, resource: &RobloxResource) -> VerificationStatus;
}

/// Default verifier that calls the Roblox API to confirm resource existence.
pub struct RobloxLiveStateVerifier<'a> {
    api: &'a RobloxApi,
}

impl<'a> RobloxLiveStateVerifier<'a> {
    pub fn new(api: &'a RobloxApi) -> Self {
        Self { api }
    }
}

fn classify<T>(result: Result<T, RobloxApiError>) -> VerificationStatus {
    match result {
        Ok(_) => VerificationStatus::Verified,
        Err(RobloxApiError::Roblox {
            status_code,
            reason,
        }) => match status_code.as_u16() {
            404 | 410 => VerificationStatus::Missing,
            401 | 403 => VerificationStatus::Unknown(format!(
                "verification not permitted ({}): {}",
                status_code, reason
            )),
            _ => VerificationStatus::Unknown(format!("{}: {}", status_code, reason)),
        },
        Err(RobloxApiError::Authorization) => {
            VerificationStatus::Unknown("authorization denied".to_owned())
        }
        Err(e) => VerificationStatus::Unknown(e.to_string()),
    }
}

#[async_trait]
impl<'a> LiveStateVerifier for RobloxLiveStateVerifier<'a> {
    async fn verify(&self, resource: &RobloxResource) -> VerificationStatus {
        let outputs = match resource.get_outputs() {
            Some(o) => o,
            None => {
                return VerificationStatus::Skipped("resource has no persisted outputs".to_owned())
            }
        };

        // Only resource types with a stable, low-cost existence endpoint are
        // verified live. Other types (configurations, ordering, sub-resources
        // implied by a parent) are intentionally classified as Skipped — their
        // drift is implied by the verification of their parent.
        match outputs {
            RobloxOutputs::Experience(o) => classify(self.api.get_experience(o.asset_id).await),
            RobloxOutputs::Place(o) => classify(self.api.get_place(o.asset_id).await),
            RobloxOutputs::Pass(o) => classify(self.api.get_game_pass(o.asset_id).await),
            RobloxOutputs::Product(o) => {
                classify(self.api.get_developer_product(o.product_id).await)
            }
            RobloxOutputs::ExperienceConfiguration
            | RobloxOutputs::ExperienceActivation
            | RobloxOutputs::ExperienceIcon(_)
            | RobloxOutputs::ExperienceThumbnail(_)
            | RobloxOutputs::ExperienceThumbnailOrder
            | RobloxOutputs::PlaceFile(_)
            | RobloxOutputs::PlaceConfiguration
            | RobloxOutputs::SocialLink(_)
            | RobloxOutputs::ProductIcon(_)
            | RobloxOutputs::Badge(_)
            | RobloxOutputs::BadgeIcon(_)
            | RobloxOutputs::ImageAsset(_)
            | RobloxOutputs::AudioAsset(_)
            | RobloxOutputs::AssetAlias(_)
            | RobloxOutputs::SpatialVoice
            | RobloxOutputs::Notification(_) => VerificationStatus::Skipped(
                "no live verification endpoint for this resource type".to_owned(),
            ),
        }
    }
}

/// Walk a previous-state graph and apply [`LiveStateVerifier`] to each
/// resource, returning a [`ReconciliationReport`].
pub async fn verify_graph<V: LiveStateVerifier>(
    previous_graph: &ResourceGraph<RobloxResource, RobloxInputs, RobloxOutputs>,
    verifier: &V,
) -> ReconciliationReport {
    let mut report = ReconciliationReport::new();
    for resource in previous_graph.get_resource_list() {
        let status = verifier.verify(&resource).await;
        report.entries.insert(resource.get_id(), status);
    }
    report
}

/// Pure helper: given a previous-state graph and a [`ReconciliationReport`],
/// produce a reconciled previous-state graph by removing resources that
/// are confirmed missing on the live platform. Resources whose status is
/// `Verified`, `Skipped`, or `Unknown` are preserved unchanged so that
/// downstream diff/apply behavior remains stable when verification is
/// inconclusive.
pub fn reconcile_graph_with_statuses(
    previous_graph: &ResourceGraph<RobloxResource, RobloxInputs, RobloxOutputs>,
    report: &ReconciliationReport,
) -> ResourceGraph<RobloxResource, RobloxInputs, RobloxOutputs> {
    let kept: Vec<RobloxResource> = previous_graph
        .get_resource_list()
        .into_iter()
        .filter(|r| {
            !matches!(
                report.entries.get(&r.get_id()),
                Some(VerificationStatus::Missing)
            )
        })
        .collect();
    ResourceGraph::new(&kept)
}

/// Convenience wrapper: verify and produce both the reconciled graph and
/// the report in a single call.
pub async fn reconcile_graph<V: LiveStateVerifier>(
    previous_graph: &ResourceGraph<RobloxResource, RobloxInputs, RobloxOutputs>,
    verifier: &V,
) -> (
    ResourceGraph<RobloxResource, RobloxInputs, RobloxOutputs>,
    ReconciliationReport,
) {
    let report = verify_graph(previous_graph, verifier).await;
    let reconciled = reconcile_graph_with_statuses(previous_graph, &report);
    (reconciled, report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roblox_resource_manager::{
        outputs::{AssetOutputs, ExperienceOutputs},
        RobloxInputs,
    };

    fn make_resource(
        id: &str,
        outputs: Option<RobloxOutputs>,
        deps: &[&RobloxResource],
    ) -> RobloxResource {
        match outputs {
            Some(o) => RobloxResource::existing(
                id,
                RobloxInputs::ExperienceActivation(
                    crate::roblox_resource_manager::ExperienceActivationInputs { is_active: true },
                ),
                o,
                deps,
            ),
            None => RobloxResource::new(
                id,
                RobloxInputs::ExperienceActivation(
                    crate::roblox_resource_manager::ExperienceActivationInputs { is_active: true },
                ),
                deps,
            ),
        }
    }

    fn experience_outputs(id: u64) -> RobloxOutputs {
        RobloxOutputs::Experience(ExperienceOutputs {
            asset_id: id,
            start_place_id: id + 1,
        })
    }

    fn place_outputs(id: u64) -> RobloxOutputs {
        RobloxOutputs::Place(AssetOutputs { asset_id: id })
    }

    struct FixedVerifier(BTreeMap<String, VerificationStatus>);

    #[async_trait]
    impl LiveStateVerifier for FixedVerifier {
        async fn verify(&self, resource: &RobloxResource) -> VerificationStatus {
            self.0
                .get(&resource.get_id())
                .cloned()
                .unwrap_or(VerificationStatus::Verified)
        }
    }

    #[tokio::test]
    async fn missing_resources_are_dropped_from_reconciled_graph() {
        let exp = make_resource("experience", Some(experience_outputs(1)), &[]);
        let place = make_resource("place_start", Some(place_outputs(2)), &[&exp]);
        let prev = ResourceGraph::new(&[exp, place]);

        let mut statuses = BTreeMap::new();
        statuses.insert("experience".into(), VerificationStatus::Verified);
        statuses.insert("place_start".into(), VerificationStatus::Missing);
        let verifier = FixedVerifier(statuses);

        let (reconciled, report) = reconcile_graph(&prev, &verifier).await;

        assert!(report.has_drift());
        assert_eq!(report.missing(), vec![&"place_start".to_string()]);
        assert!(reconciled.get_outputs("experience").is_some());
        assert!(reconciled.get_outputs("place_start").is_none());
    }

    #[tokio::test]
    async fn unknown_and_skipped_resources_are_preserved() {
        let exp = make_resource("experience", Some(experience_outputs(1)), &[]);
        let place = make_resource("place_start", Some(place_outputs(2)), &[&exp]);
        let prev = ResourceGraph::new(&[exp, place]);

        let mut statuses = BTreeMap::new();
        statuses.insert(
            "experience".into(),
            VerificationStatus::Unknown("network".into()),
        );
        statuses.insert(
            "place_start".into(),
            VerificationStatus::Skipped("n/a".into()),
        );
        let verifier = FixedVerifier(statuses);

        let (reconciled, report) = reconcile_graph(&prev, &verifier).await;

        assert!(!report.has_drift());
        assert!(reconciled.get_outputs("experience").is_some());
        assert!(reconciled.get_outputs("place_start").is_some());
        let counts = report.counts();
        assert_eq!(counts.unknown, 1);
        assert_eq!(counts.skipped, 1);
        assert_eq!(counts.missing, 0);
    }

    #[tokio::test]
    async fn classify_maps_404_to_missing() {
        use reqwest::StatusCode;
        let s = classify::<()>(Err(RobloxApiError::Roblox {
            status_code: StatusCode::NOT_FOUND,
            reason: "not found".into(),
        }));
        assert_eq!(s, VerificationStatus::Missing);
    }

    #[tokio::test]
    async fn classify_maps_403_to_unknown() {
        use reqwest::StatusCode;
        let s = classify::<()>(Err(RobloxApiError::Roblox {
            status_code: StatusCode::FORBIDDEN,
            reason: "forbidden".into(),
        }));
        assert!(matches!(s, VerificationStatus::Unknown(_)));
    }

    #[tokio::test]
    async fn classify_maps_ok_to_verified() {
        let s = classify::<u32>(Ok(0));
        assert_eq!(s, VerificationStatus::Verified);
    }
}

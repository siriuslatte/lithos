//! Project state: load, save, and reconstruct the resource graph.
//!
//! Internally split into:
//! - [`io`][]: persistence (file/S3 IO, versioning, parse/serialize)
//! - [`build`][]: graph construction (from config or by importing live data)

mod aws_credentials_provider;
mod build;
mod history;
mod io;
mod legacy_resources;
mod progress;
pub mod reconcile;
pub mod v1;
pub mod v2;
pub mod v3;
pub mod v4;
pub mod v5;
pub mod v6;
pub mod v7;

pub use build::{get_desired_graph, import_graph};
pub use history::{
    build_failure_journal, build_success_journal, latest_deployment_diagnostics, rollback_snapshot,
};
pub use io::{
    get_previous_state, get_state, get_state_from_source, save_state, save_state_to_file,
    save_state_to_remote, ResourceStateVLatest,
};
pub use progress::DeploymentProgressWriter;
pub use reconcile::{
    reconcile_graph, reconcile_graph_with_statuses, verify_graph, LiveStateVerifier,
    ReconciliationCounts, ReconciliationReport, RobloxLiveStateVerifier, VerificationStatus,
};

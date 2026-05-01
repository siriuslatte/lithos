use std::{error::Error, fmt, fs, path::Path};

use rbx_api::{
    api_keys::models::IntrospectApiKeyResponse, errors::RobloxApiError, models::AssetId,
};
use serde::{Deserialize, Serialize};

use crate::{
    config::{OwnerConfig, PlayabilityTargetConfig, TargetConfig},
    resource_graph::{Resource, ResourceGraph},
    roblox_resource_manager::{RobloxInputs, RobloxOutputs, RobloxResource},
};

const ROBLOX_PLACE_SIZE_LIMIT_BYTES: u64 = 100 * 1024 * 1024;
const ROBLOX_PLACE_WARNING_BYTES: u64 = 90 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
}

impl OperationContext {
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticCategory {
    Preflight,
    Authentication,
    Authorization,
    Ownership,
    Configuration,
    File,
    Roblox,
    Rollback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticConfidence {
    Confirmed,
    High,
    Medium,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationAction {
    Preflight,
    Create,
    Update,
    Delete,
    Rollback,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationContext {
    pub action: OperationAction,
    pub resource_id: Option<String>,
    pub resource_type: String,
    pub auth_model: String,
    pub creator_target: Option<String>,
    pub endpoint: Option<String>,
    pub file_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentDiagnostic {
    pub code: String,
    pub severity: DiagnosticSeverity,
    pub category: DiagnosticCategory,
    pub confidence: DiagnosticConfidence,
    pub summary: String,
    pub detail: Option<String>,
    #[serde(default)]
    pub probable_causes: Vec<String>,
    #[serde(default)]
    pub next_steps: Vec<String>,
    pub operation: Option<OperationContext>,
}

impl DeploymentDiagnostic {
    pub fn error(code: &str, category: DiagnosticCategory, summary: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            severity: DiagnosticSeverity::Error,
            category,
            confidence: DiagnosticConfidence::Confirmed,
            summary: summary.into(),
            detail: None,
            probable_causes: Vec::new(),
            next_steps: Vec::new(),
            operation: None,
        }
    }

    pub fn warning(code: &str, category: DiagnosticCategory, summary: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            severity: DiagnosticSeverity::Warning,
            category,
            confidence: DiagnosticConfidence::Medium,
            summary: summary.into(),
            detail: None,
            probable_causes: Vec::new(),
            next_steps: Vec::new(),
            operation: None,
        }
    }

    pub fn with_confidence(mut self, confidence: DiagnosticConfidence) -> Self {
        self.confidence = confidence;
        self
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn with_probable_causes(mut self, probable_causes: Vec<String>) -> Self {
        self.probable_causes = probable_causes;
        self
    }

    pub fn with_next_steps(mut self, next_steps: Vec<String>) -> Self {
        self.next_steps = next_steps;
        self
    }

    pub fn with_operation(mut self, operation: OperationContext) -> Self {
        self.operation = Some(operation);
        self
    }
}

#[derive(Debug, Clone)]
pub struct OperationError {
    summary: String,
    diagnostics: Vec<DeploymentDiagnostic>,
}

impl OperationError {
    pub fn new(summary: impl Into<String>, diagnostics: Vec<DeploymentDiagnostic>) -> Self {
        Self {
            summary: summary.into(),
            diagnostics,
        }
    }

    pub fn from_diagnostic(diagnostic: DeploymentDiagnostic) -> Self {
        let summary = diagnostic.summary.clone();
        Self::new(summary, vec![diagnostic])
    }

    pub fn summary(&self) -> &str {
        &self.summary
    }

    pub fn diagnostics(&self) -> &[DeploymentDiagnostic] {
        &self.diagnostics
    }
}

impl fmt::Display for OperationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.summary)
    }
}

impl Error for OperationError {}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticReport {
    #[serde(default)]
    pub blocking: Vec<DeploymentDiagnostic>,
    #[serde(default)]
    pub warnings: Vec<DeploymentDiagnostic>,
    #[serde(default)]
    pub info: Vec<DeploymentDiagnostic>,
}

impl DiagnosticReport {
    pub fn push(&mut self, diagnostic: DeploymentDiagnostic) {
        match diagnostic.severity {
            DiagnosticSeverity::Error => self.blocking.push(diagnostic),
            DiagnosticSeverity::Warning => self.warnings.push(diagnostic),
            DiagnosticSeverity::Info => self.info.push(diagnostic),
        }
    }

    pub fn has_blocking(&self) -> bool {
        !self.blocking.is_empty()
    }
}

#[derive(Debug, Clone, Default)]
pub struct PreflightAuthContext {
    pub open_cloud_api_key_present: bool,
    pub open_cloud_key: Option<IntrospectApiKeyResponse>,
}

pub fn run_preflight_checks(
    project_path: &Path,
    target_config: &TargetConfig,
    owner_config: &OwnerConfig,
    current_graph: &ResourceGraph<RobloxResource, RobloxInputs, RobloxOutputs>,
    next_graph: &ResourceGraph<RobloxResource, RobloxInputs, RobloxOutputs>,
    auth: &PreflightAuthContext,
) -> DiagnosticReport {
    let mut report = DiagnosticReport::default();

    validate_target_configuration(target_config, owner_config, &mut report);

    let existing_experience_id = match current_graph.get_outputs("experience_singleton") {
        Some(RobloxOutputs::Experience(outputs)) => Some(outputs.asset_id),
        _ => None,
    };

    let mut requires_open_cloud_place_publish = false;

    for resource in next_graph.get_resource_list() {
        let resource_id = resource.get_id();
        match resource.get_inputs() {
            RobloxInputs::PlaceFile(file) => {
                requires_open_cloud_place_publish = true;
                validate_place_file(project_path, &resource_id, &file.file_path, &mut report);
            }
            RobloxInputs::ExperienceIcon(file)
            | RobloxInputs::ExperienceThumbnail(file)
            | RobloxInputs::ProductIcon(file)
            | RobloxInputs::BadgeIcon(file) => {
                validate_file_extension(
                    project_path,
                    &resource_id,
                    &file.file_path,
                    &["png", "jpg", "jpeg", "bmp", "gif", "tga"],
                    "image",
                    &mut report,
                );
            }
            RobloxInputs::Pass(file) => {
                validate_file_extension(
                    project_path,
                    &resource_id,
                    &file.icon_file_path,
                    &["png", "jpg", "jpeg", "bmp", "gif", "tga"],
                    "image",
                    &mut report,
                );
            }
            RobloxInputs::Badge(file) => {
                validate_file_extension(
                    project_path,
                    &resource_id,
                    &file.icon_file_path,
                    &["png", "jpg", "jpeg", "bmp", "gif", "tga"],
                    "image",
                    &mut report,
                );
            }
            RobloxInputs::ImageAsset(file) => {
                validate_file_extension(
                    project_path,
                    &resource_id,
                    &file.file_path,
                    &["png", "jpg", "jpeg", "bmp", "gif", "tga"],
                    "image",
                    &mut report,
                );
            }
            RobloxInputs::AudioAsset(file) => {
                validate_file_extension(
                    project_path,
                    &resource_id,
                    &file.file_path,
                    &["ogg", "mp3"],
                    "audio",
                    &mut report,
                );
            }
            _ => {}
        }
    }

    if requires_open_cloud_place_publish {
        validate_place_publish_auth(existing_experience_id, auth, &mut report);
    }

    report
}

pub fn map_roblox_api_error(context: OperationContext, error: &RobloxApiError) -> OperationError {
    match error {
        RobloxApiError::Authorization => OperationError::from_diagnostic(
            DeploymentDiagnostic::error(
                "roblosecurity-authorization-denied",
                DiagnosticCategory::Authentication,
                format!(
                    "Roblox denied the authenticated session while trying to {} {}.",
                    action_label(context.action),
                    context.resource_type
                ),
            )
            .with_confidence(DiagnosticConfidence::High)
            .with_detail(format!(
                "The request was redirected to Roblox login instead of completing{}.{}",
                context
                    .endpoint
                    .as_ref()
                    .map(|endpoint| format!(" via {}", endpoint))
                    .unwrap_or_default(),
                context_detail_suffix(&context),
            ))
            .with_next_steps(vec![
                "Refresh the ROBLOSECURITY cookie used for authentication.".to_owned(),
                "If this deploy should run headlessly, verify that the CI secret still contains a valid cookie and that the account still has access to the target creator.".to_owned(),
            ])
            .with_operation(context),
        ),
        RobloxApiError::Roblox {
            status_code,
            reason,
        } => classify_roblox_response(context, *status_code, reason),
        RobloxApiError::InvalidFileExtension(path) => OperationError::from_diagnostic(
            DeploymentDiagnostic::error(
                "invalid-file-extension",
                DiagnosticCategory::File,
                format!("Roblox rejected an unsupported file extension for {}.", path),
            )
            .with_detail(format!(
                "Lithos attempted to {} {}, but the file extension is not supported.{}",
                action_label(context.action),
                context.resource_type,
                context_detail_suffix(&context),
            ))
            .with_next_steps(vec![
                "Use one of Roblox's supported file formats for this resource type.".to_owned(),
                "Re-run `lithos deploy` after fixing the file path or extension.".to_owned(),
            ])
            .with_operation(context),
        ),
        RobloxApiError::NoFileName(path) => OperationError::from_diagnostic(
            DeploymentDiagnostic::error(
                "missing-file-name",
                DiagnosticCategory::File,
                format!("Roblox could not determine a file name for {}.", path),
            )
            .with_detail(format!(
                "The path could not be converted into a valid Roblox upload file name.{}",
                context_detail_suffix(&context),
            ))
            .with_next_steps(vec![
                "Use a normal file path with a file name and extension.".to_owned(),
            ])
            .with_operation(context),
        ),
        RobloxApiError::ReadFile(error) => OperationError::from_diagnostic(
            DeploymentDiagnostic::error(
                "read-file-failed",
                DiagnosticCategory::File,
                format!("Lithos could not read a local file while preparing {}.", context.resource_type),
            )
            .with_detail(format!("{}{}", error, context_detail_suffix(&context)))
            .with_next_steps(vec![
                "Restore the referenced file and verify the process still has permission to read it.".to_owned(),
            ])
            .with_operation(context),
        ),
        RobloxApiError::RbxlxPlaceFileSizeTooLarge
        | RobloxApiError::RbxlPlaceFileSizeTooLarge
        | RobloxApiError::RbxlxPlaceFileSizeMayBeTooLarge
        | RobloxApiError::RbxlPlaceFileSizeMayBeTooLarge => map_operation_message(context, &error.to_string()),
        RobloxApiError::Other(error) => map_operation_message(context, &error.to_string()),
        other => OperationError::from_diagnostic(
            DeploymentDiagnostic::error(
                "roblox-request-failed",
                DiagnosticCategory::Roblox,
                format!(
                    "Roblox failed while trying to {} {}.",
                    action_label(context.action),
                    context.resource_type
                ),
            )
            .with_confidence(DiagnosticConfidence::High)
            .with_detail(format!("{}{}", other, context_detail_suffix(&context)))
            .with_next_steps(vec![
                "Retry the deploy after verifying authentication and creator permissions.".to_owned(),
            ])
            .with_operation(context),
        ),
    }
}

pub fn map_operation_message(context: OperationContext, message: &str) -> OperationError {
    let lower = message.to_ascii_lowercase();

    if lower.contains("open cloud authentication") {
        return OperationError::from_diagnostic(
            DeploymentDiagnostic::error(
                "missing-open-cloud-authentication",
                DiagnosticCategory::Authentication,
                "Place publishing requires an Open Cloud API key.".to_owned(),
            )
            .with_detail(format!("{}{}", message, context_detail_suffix(&context)))
            .with_next_steps(vec![
                "Create or rotate a Roblox API key with `universe-places.write`.".to_owned(),
                "Set `LITHOS_OPEN_CLOUD_API_KEY` before rerunning deploy. Lithos also accepts `ROBLOX_OPEN_CLOUD_API_KEY`.".to_owned(),
            ])
            .with_operation(context),
        );
    }

    if lower.contains("quota") && context.resource_type == "AudioAsset" {
        return OperationError::from_diagnostic(
            DeploymentDiagnostic::error(
                "audio-upload-quota-exhausted",
                DiagnosticCategory::Roblox,
                "Roblox audio upload quota is exhausted for the current period.",
            )
            .with_detail(format!("{}{}", message, context_detail_suffix(&context)))
            .with_next_steps(vec![
                "Wait for the Roblox audio quota window to reset or use a different creator account with available quota.".to_owned(),
            ])
            .with_operation(context),
        );
    }

    if context.resource_type == "PlaceFile"
        && (lower.contains("403")
            || lower.contains("401")
            || lower.contains("forbidden")
            || lower.contains("permission"))
    {
        return OperationError::from_diagnostic(place_publish_permission_diagnostic(
            context,
            Some(message.to_owned()),
            DiagnosticConfidence::High,
        ));
    }

    if context.resource_type == "PlaceFile"
        && (lower.contains("internal server error")
            || lower.contains("unknown error")
            || lower.contains("500"))
    {
        return OperationError::from_diagnostic(place_publish_vague_server_diagnostic(
            context,
            Some(message.to_owned()),
        ));
    }

    OperationError::from_diagnostic(
        DeploymentDiagnostic::error(
            "operation-failed",
            DiagnosticCategory::Roblox,
            format!(
                "{} {} failed.",
                action_label(context.action).to_uppercase_first(),
                context.resource_type
            ),
        )
        .with_confidence(DiagnosticConfidence::High)
        .with_detail(format!("{}{}", message, context_detail_suffix(&context)))
        .with_next_steps(vec![
            "Review the resource-specific error details above and retry the deploy after correcting the underlying Roblox-side issue.".to_owned(),
        ])
        .with_operation(context),
    )
}

fn classify_roblox_response(
    context: OperationContext,
    status_code: reqwest::StatusCode,
    reason: &str,
) -> OperationError {
    let status = status_code.as_u16();
    let lower_reason = reason.to_ascii_lowercase();

    if context.resource_type == "PlaceFile"
        && (status == 401 || status == 403 || lower_reason.contains("permission"))
    {
        return OperationError::from_diagnostic(place_publish_permission_diagnostic(
            context,
            Some(format!("{}: {}", status_code, reason)),
            DiagnosticConfidence::Confirmed,
        ));
    }

    if context.resource_type == "PlaceFile"
        && (status >= 500
            || lower_reason.contains("internal server error")
            || lower_reason.contains("unknown error"))
    {
        return OperationError::from_diagnostic(place_publish_vague_server_diagnostic(
            context,
            Some(format!("{}: {}", status_code, reason)),
        ));
    }

    if context.resource_type == "Experience"
        && context.action == OperationAction::Create
        && status >= 500
        && context
            .creator_target
            .as_deref()
            .map(|value| value.starts_with("group:"))
            .unwrap_or(false)
    {
        return OperationError::from_diagnostic(
            DeploymentDiagnostic::error(
                "group-experience-create-vague-failure",
                DiagnosticCategory::Roblox,
                "Roblox returned a vague server failure while creating a group-owned experience.",
            )
            .with_confidence(DiagnosticConfidence::High)
            .with_detail(format!(
                "{}: {}{}",
                status_code,
                reason,
                context_detail_suffix(&context),
            ))
            .with_probable_causes(vec![
                "The authenticated account is missing the group permission required to create or edit group experiences.".to_owned(),
                "Roblox is blocking the operation behind a group-specific prerequisite, such as role permissions or legal/terms acceptance, but only surfaced a generic server error.".to_owned(),
            ])
            .with_next_steps(vec![
                "Verify that the account can manually create or edit the target group's experiences in Creator Dashboard or Studio.".to_owned(),
                "If this is new group automation, confirm the group has accepted any required Roblox legal terms and that the automation account has the correct group role.".to_owned(),
            ])
            .with_operation(context),
        );
    }

    let category = if status == 401 || status == 403 {
        DiagnosticCategory::Authorization
    } else {
        DiagnosticCategory::Roblox
    };
    let confidence = if status >= 500 || lower_reason.contains("unknown error") {
        DiagnosticConfidence::High
    } else {
        DiagnosticConfidence::Confirmed
    };

    OperationError::from_diagnostic(
        DeploymentDiagnostic::error(
            "roblox-response-failed",
            category,
            format!(
                "Roblox returned {} while trying to {} {}.",
                status_code,
                action_label(context.action),
                context.resource_type
            ),
        )
        .with_confidence(confidence)
        .with_detail(format!("{}{}", reason, context_detail_suffix(&context)))
        .with_next_steps(vec![
            "Check creator permissions, ownership, and resource configuration for this operation.".to_owned(),
            "Retry once after correcting the issue. If the response stays vague, validate the same action manually in Creator Dashboard or Studio to identify any platform-side gating.".to_owned(),
        ])
        .with_operation(context),
    )
}

fn place_publish_permission_diagnostic(
    context: OperationContext,
    detail: Option<String>,
    confidence: DiagnosticConfidence,
) -> DeploymentDiagnostic {
    let detail = detail.unwrap_or_else(|| "Roblox denied the place publishing request.".to_owned());

    DeploymentDiagnostic::error(
        "place-publish-permission-denied",
        DiagnosticCategory::Authorization,
        "Roblox denied the place publishing request.",
    )
    .with_confidence(confidence)
    .with_detail(format!("{}{}", detail, context_detail_suffix(&context)))
    .with_probable_causes(vec![
        "The Open Cloud API key is missing `universe-places.write` for the target experience.".to_owned(),
        "The API key is restricted to a different experience or blocked by an IP allowlist.".to_owned(),
        "The account behind the API key no longer has permission to publish to the target creator or group.".to_owned(),
    ])
    .with_next_steps(vec![
        "Run Lithos preflight and confirm the key includes `universe-places.write` for this universe.".to_owned(),
        "Verify the automation account can publish the same place manually or via Creator Dashboard/Studio.".to_owned(),
    ])
    .with_operation(context)
}

fn place_publish_vague_server_diagnostic(
    context: OperationContext,
    detail: Option<String>,
) -> DeploymentDiagnostic {
    let detail = detail.unwrap_or_else(|| {
        "Roblox returned a vague server error while publishing the place.".to_owned()
    });

    DeploymentDiagnostic::error(
        "place-publish-vague-server-error",
        DiagnosticCategory::Roblox,
        "Roblox returned a vague server error while publishing the place.",
    )
    .with_confidence(DiagnosticConfidence::High)
    .with_detail(format!("{}{}", detail, context_detail_suffix(&context)))
    .with_probable_causes(vec![
        "The place publish request is failing behind a generic Roblox server response, often because of missing Open Cloud access, creator permission mismatches, or platform-side gating.".to_owned(),
        "The place file may contain Roblox instance types the Place Publishing API does not update, such as EditableImage, EditableMesh, PartOperation, SurfaceAppearance, or BaseWrap.".to_owned(),
        "The place file may be close to Roblox's practical upload limit or require Studio publishing instead of API publishing.".to_owned(),
    ])
    .with_next_steps(vec![
        "Verify the Open Cloud key scope and universe restriction first.".to_owned(),
        "Check the place for unsupported instance types and large file size, then retry.".to_owned(),
        "If the same publish succeeds in Studio but not via API, treat it as a platform-side place publishing limitation and keep the rollout manual for that content.".to_owned(),
    ])
    .with_operation(context)
}

fn action_label(action: OperationAction) -> &'static str {
    match action {
        OperationAction::Preflight => "preflight",
        OperationAction::Create => "create",
        OperationAction::Update => "update",
        OperationAction::Delete => "delete",
        OperationAction::Rollback => "rollback",
    }
}

fn context_detail_suffix(context: &OperationContext) -> String {
    let mut parts = Vec::new();
    if let Some(resource_id) = &context.resource_id {
        parts.push(format!("resource {}", resource_id));
    }
    if let Some(endpoint) = &context.endpoint {
        parts.push(format!("endpoint {}", endpoint));
    }
    if let Some(file_path) = &context.file_path {
        parts.push(format!("file {}", file_path));
    }
    if let Some(creator_target) = &context.creator_target {
        parts.push(format!("creator {}", creator_target));
    }

    if parts.is_empty() {
        String::new()
    } else {
        format!(" [{}]", parts.join(", "))
    }
}
fn validate_target_configuration(
    target_config: &TargetConfig,
    owner_config: &OwnerConfig,
    report: &mut DiagnosticReport,
) {
    let TargetConfig::Experience(target_config) = target_config;
    let playability = target_config
        .configuration
        .as_ref()
        .and_then(|config| config.playability);

    if matches!(owner_config, OwnerConfig::Group(_))
        && matches!(playability, Some(PlayabilityTargetConfig::Friends))
    {
        report.push(
            DeploymentDiagnostic::error(
                "group-friends-access-unsupported",
                DiagnosticCategory::Configuration,
                "Group-owned experiences cannot use `playability: friends`.",
            )
            .with_detail(
                "Roblox only exposes the Friends audience for user-owned experiences. Group-owned experiences must stay private, become public, or use the group-only audience in Creator Dashboard.",
            )
            .with_next_steps(vec![
                "Change the environment or target playability to `private` or `public`.".to_owned(),
                "If you need a group-only rollout, configure access in Creator Dashboard after deploy.".to_owned(),
            ]),
        );
    }

    if matches!(playability, Some(PlayabilityTargetConfig::Public)) {
        report.push(
            DeploymentDiagnostic::warning(
                "public-experience-gating",
                DiagnosticCategory::Preflight,
                "Public experience updates can still be blocked by Roblox account and compliance requirements.",
            )
            .with_detail(
                "Roblox documents additional gating for public experiences, including content maturity/compliance, account age, and account verification or purchase history. Lithos cannot verify those requirements ahead of time.",
            )
            .with_probable_causes(vec![
                "The required content maturity questionnaire is incomplete for the experience.".to_owned(),
                "The publishing account does not satisfy Roblox's eligibility requirements for public publishing.".to_owned(),
            ])
            .with_next_steps(vec![
                "Confirm the experience can be switched to Public in Creator Dashboard before running CI deploys.".to_owned(),
                "If Roblox later returns a vague 4xx/5xx, check the experience privacy settings, content maturity questionnaire, and account eligibility requirements first.".to_owned(),
            ]),
        );
    }
}

fn validate_place_publish_auth(
    existing_experience_id: Option<AssetId>,
    auth: &PreflightAuthContext,
    report: &mut DiagnosticReport,
) {
    if !auth.open_cloud_api_key_present {
        report.push(
            DeploymentDiagnostic::error(
                "missing-open-cloud-key",
                DiagnosticCategory::Authentication,
                "Place publishing requires an Open Cloud API key (`LITHOS_OPEN_CLOUD_API_KEY` is preferred).",
            )
            .with_detail(
                "Lithos uses Roblox's Place Publishing API for place file uploads. Without an Open Cloud API key, deploy cannot publish `.rbxl` or `.rbxlx` files.",
            )
            .with_next_steps(vec![
                "Create an API key with the `universe-places` API and the `write` operation.".to_owned(),
                "Export it as `LITHOS_OPEN_CLOUD_API_KEY` before running `lithos deploy`. Lithos also accepts `ROBLOX_OPEN_CLOUD_API_KEY`.".to_owned(),
            ]),
        );
        return;
    }

    let Some(open_cloud_key) = auth.open_cloud_key.as_ref() else {
        report.push(
            DeploymentDiagnostic::warning(
                "open-cloud-introspection-unavailable",
                DiagnosticCategory::Preflight,
                "Lithos could not verify the Open Cloud API key's scopes ahead of time.",
            )
            .with_confidence(DiagnosticConfidence::High)
            .with_detail(
                "The key is present, but preflight could not introspect its status or scope selection. Deploy will continue and classify any Roblox permission errors with more specific guidance if publishing fails.",
            ),
        );
        return;
    };

    if !open_cloud_key.enabled {
        report.push(
            DeploymentDiagnostic::error(
                "open-cloud-key-disabled",
                DiagnosticCategory::Authentication,
                "The configured Open Cloud API key is disabled.",
            )
            .with_detail(format!(
                "Roblox reported that API key '{}' is disabled.",
                open_cloud_key.name
            ))
            .with_next_steps(vec![
                "Enable the API key in Creator Dashboard or replace it with an active key."
                    .to_owned(),
            ]),
        );
    }

    if open_cloud_key.expired {
        report.push(
            DeploymentDiagnostic::error(
                "open-cloud-key-expired",
                DiagnosticCategory::Authentication,
                "The configured Open Cloud API key is expired.",
            )
            .with_detail(format!(
                "Roblox reported that API key '{}' is expired or auto-expired.",
                open_cloud_key.name
            ))
            .with_next_steps(vec![
                "Rotate or regenerate the API key in Creator Dashboard and update `LITHOS_OPEN_CLOUD_API_KEY` (or the legacy alias you are using).".to_owned(),
            ]),
        );
    }

    if !open_cloud_key.has_scope_operation("universe-places", "write") {
        report.push(
            DeploymentDiagnostic::error(
                "missing-universe-places-write-scope",
                DiagnosticCategory::Authorization,
                "The Open Cloud API key cannot publish place versions.",
            )
            .with_detail(
                "Roblox API key introspection did not find the `universe-places` API with the `write` operation.",
            )
            .with_next_steps(vec![
                "Edit the API key in Creator Dashboard and add the `universe-places` API with `write` access.".to_owned(),
            ]),
        );
        return;
    }

    match existing_experience_id {
        Some(experience_id)
            if !open_cloud_key.allows_universe_operation(
                "universe-places",
                "write",
                experience_id,
            ) =>
        {
            report.push(
                DeploymentDiagnostic::error(
                    "api-key-missing-target-universe",
                    DiagnosticCategory::Authorization,
                    "The Open Cloud API key does not include the target experience for place publishing.",
                )
                .with_detail(format!(
                    "Roblox API key introspection found `universe-places.write`, but not for universe {}.",
                    experience_id
                ))
                .with_next_steps(vec![
                    "Edit the API key's experience restrictions and add the target universe, or disable the experience restriction for this key.".to_owned(),
                ]),
            );
        }
        None if open_cloud_key.has_scope_operation("universe-places", "write")
            && !open_cloud_key.has_wildcard_universe_operation("universe-places", "write") =>
        {
            report.push(
                DeploymentDiagnostic::warning(
                    "api-key-new-universe-scope-inconclusive",
                    DiagnosticCategory::Preflight,
                    "Lithos cannot prove that the Open Cloud API key will cover a newly created experience.",
                )
                .with_confidence(DiagnosticConfidence::High)
                .with_detail(
                    "The API key has `universe-places.write`, but its scope is restricted to specific experiences. If this deploy creates a brand-new universe, the subsequent place publish may still fail until the key is updated.",
                )
                .with_next_steps(vec![
                    "If this deploy creates a new experience, prefer a wildcard or newly updated API key before running automation.".to_owned(),
                ]),
            );
        }
        _ => {}
    }
}

fn validate_place_file(
    project_path: &Path,
    resource_id: &str,
    file_path: &str,
    report: &mut DiagnosticReport,
) {
    let full_path = project_path.join(file_path);
    let extension = extension_lowercase(&full_path);

    if !matches!(extension.as_deref(), Some("rbxl" | "rbxlx")) {
        report.push(
            DeploymentDiagnostic::error(
                "invalid-place-file-extension",
                DiagnosticCategory::File,
                format!(
                    "{} points to a file that is not `.rbxl` or `.rbxlx`.",
                    resource_id
                ),
            )
            .with_detail(format!(
                "Lithos can only publish Roblox place files. Received '{}'.",
                file_path
            ))
            .with_operation(file_operation_context(
                resource_id,
                "PlaceFile",
                file_path,
            )),
        );
        return;
    }

    let Some(metadata) = read_metadata(&full_path, resource_id, "PlaceFile", file_path, report)
    else {
        return;
    };

    if metadata.len() > ROBLOX_PLACE_SIZE_LIMIT_BYTES {
        let summary = match extension.as_deref() {
            Some("rbxlx") => {
                format!(
                    "{} exceeds Roblox's documented 100 MB place publishing limit.",
                    file_path
                )
            }
            _ => format!(
                "{} exceeds Roblox's documented 100 MB place publishing limit.",
                file_path
            ),
        };
        let detail = match extension.as_deref() {
            Some("rbxlx") => "Roblox documents a 100 MB limit for published places. XML place files are usually larger than their binary `.rbxl` equivalent and are more likely to hit this limit.".to_owned(),
            _ => "Roblox documents a 100 MB limit for published places. Files above that limit often fail with vague server errors during publish.".to_owned(),
        };

        report.push(
            DeploymentDiagnostic::error(
                "place-file-too-large",
                DiagnosticCategory::File,
                summary,
            )
            .with_detail(detail)
            .with_next_steps(vec![
                "Reduce the size of the place file before deploying.".to_owned(),
                "If the file is `.rbxlx`, export a binary `.rbxl` build and deploy that instead.".to_owned(),
            ])
            .with_operation(file_operation_context(resource_id, "PlaceFile", file_path)),
        );
        return;
    }

    if metadata.len() >= ROBLOX_PLACE_WARNING_BYTES {
        report.push(
            DeploymentDiagnostic::warning(
                "place-file-near-limit",
                DiagnosticCategory::File,
                format!("{} is close to Roblox's 100 MB place publishing limit.", file_path),
            )
            .with_confidence(DiagnosticConfidence::High)
            .with_detail(match extension.as_deref() {
                Some("rbxlx") => "Large `.rbxlx` files frequently hit publishing limits before the raw file size looks extreme. Roblox recommends using `.rbxl` when size becomes an issue.".to_owned(),
                _ => "Large place files can fail during publish with vague server responses even before they cross the documented hard limit.".to_owned(),
            })
            .with_operation(file_operation_context(resource_id, "PlaceFile", file_path)),
        );
    }
}

fn validate_file_extension(
    project_path: &Path,
    resource_id: &str,
    file_path: &str,
    allowed_extensions: &[&str],
    file_kind: &str,
    report: &mut DiagnosticReport,
) {
    let full_path = project_path.join(file_path);
    let extension = extension_lowercase(&full_path);
    if !matches!(extension.as_deref(), Some(value) if allowed_extensions.contains(&value)) {
        report.push(
            DeploymentDiagnostic::error(
                "invalid-file-extension",
                DiagnosticCategory::File,
                format!(
                    "{} must reference a supported {} file.",
                    resource_id, file_kind
                ),
            )
            .with_detail(format!(
                "Expected one of [{}] but received '{}'.",
                allowed_extensions.join(", "),
                file_path
            ))
            .with_operation(file_operation_context(
                resource_id,
                resource_id,
                file_path,
            )),
        );
        return;
    }

    let _ = read_metadata(&full_path, resource_id, resource_id, file_path, report);
}

fn read_metadata(
    full_path: &Path,
    resource_id: &str,
    resource_type: &str,
    file_path: &str,
    report: &mut DiagnosticReport,
) -> Option<fs::Metadata> {
    match fs::metadata(full_path) {
        Ok(metadata) => Some(metadata),
        Err(error) => {
            report.push(
                DeploymentDiagnostic::error(
                    "missing-local-file",
                    DiagnosticCategory::File,
                    format!("{} references a file that does not exist at apply time.", resource_id),
                )
                .with_detail(format!("{} ({})", file_path, error))
                .with_next_steps(vec![
                    "Restore the file in your working tree before deploying.".to_owned(),
                    "If you are trying to undo a deploy, check out the commit that produced the last known good snapshot first.".to_owned(),
                ])
                .with_operation(file_operation_context(resource_id, resource_type, file_path)),
            );
            None
        }
    }
}

fn extension_lowercase(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
}

fn file_operation_context(
    resource_id: &str,
    resource_type: &str,
    file_path: &str,
) -> OperationContext {
    OperationContext {
        action: OperationAction::Preflight,
        resource_id: Some(resource_id.to_owned()),
        resource_type: resource_type.to_owned(),
        auth_model: "preflight".to_owned(),
        creator_target: None,
        endpoint: None,
        file_path: Some(file_path.to_owned()),
    }
}

#[cfg(test)]
fn empty_experience_target() -> crate::config::ExperienceTargetConfig {
    crate::config::ExperienceTargetConfig {
        configuration: None,
        places: None,
        icon: None,
        thumbnails: None,
        social_links: None,
        products: None,
        passes: None,
        badges: None,
        assets: None,
        spatial_voice: None,
        notifications: None,
    }
}

trait StringCasingExt {
    fn to_uppercase_first(&self) -> String;
}

impl StringCasingExt for str {
    fn to_uppercase_first(&self) -> String {
        let mut chars = self.chars();
        match chars.next() {
            Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
            None => String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use rbx_api::api_keys::models::{ApiKeyScope, IntrospectApiKeyResponse};
    use reqwest::StatusCode;

    use super::*;
    use crate::roblox_resource_manager::{
        outputs::{AssetOutputs, ExperienceOutputs},
        ExperienceInputs, FileInputs, PlaceInputs,
    };

    struct TempProject {
        path: PathBuf,
    }

    impl TempProject {
        fn new() -> Self {
            let unique = format!(
                "lithos-diagnostics-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            );
            let path = std::env::temp_dir().join(unique);
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }

        fn write_file(&self, relative: &str, size: usize) {
            let file_path = self.path.join(relative);
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(file_path, vec![0u8; size]).unwrap();
        }
    }

    impl Drop for TempProject {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn existing_graph(
        universe_id: u64,
    ) -> ResourceGraph<RobloxResource, RobloxInputs, RobloxOutputs> {
        let experience = RobloxResource::existing(
            "experience_singleton",
            RobloxInputs::Experience(ExperienceInputs { group_id: None }),
            RobloxOutputs::Experience(ExperienceOutputs {
                asset_id: universe_id,
                start_place_id: 9001,
            }),
            &[],
        );
        let start_place = RobloxResource::existing(
            "place_start",
            RobloxInputs::Place(PlaceInputs { is_start: true }),
            RobloxOutputs::Place(AssetOutputs { asset_id: 9001 }),
            &[&experience],
        );
        ResourceGraph::new(&[experience, start_place])
    }

    fn desired_graph(
        file_path: &str,
    ) -> ResourceGraph<RobloxResource, RobloxInputs, RobloxOutputs> {
        let experience = RobloxResource::new(
            "experience_singleton",
            RobloxInputs::Experience(ExperienceInputs { group_id: None }),
            &[],
        );
        let start_place = RobloxResource::new(
            "place_start",
            RobloxInputs::Place(PlaceInputs { is_start: true }),
            &[&experience],
        );
        let place_file = RobloxResource::new(
            "placeFile_start",
            RobloxInputs::PlaceFile(FileInputs {
                file_path: file_path.to_owned(),
                file_hash: "hash".to_owned(),
            }),
            &[&start_place, &experience],
        );
        ResourceGraph::new(&[experience, start_place, place_file])
    }

    #[test]
    fn blocks_place_publish_without_open_cloud_key() {
        let temp = TempProject::new();
        temp.write_file("game.rbxl", 128);

        let report = run_preflight_checks(
            temp.path(),
            &TargetConfig::Experience(empty_experience_target()),
            &OwnerConfig::Personal,
            &existing_graph(123),
            &desired_graph("game.rbxl"),
            &PreflightAuthContext::default(),
        );

        assert!(report.has_blocking());
        assert!(report
            .blocking
            .iter()
            .any(|diagnostic| diagnostic.code == "missing-open-cloud-key"));
    }

    #[test]
    fn blocks_scope_mismatch_for_existing_universe() {
        let temp = TempProject::new();
        temp.write_file("game.rbxl", 128);

        let report = run_preflight_checks(
            temp.path(),
            &TargetConfig::Experience(empty_experience_target()),
            &OwnerConfig::Personal,
            &existing_graph(123),
            &desired_graph("game.rbxl"),
            &PreflightAuthContext {
                open_cloud_api_key_present: true,
                open_cloud_key: Some(IntrospectApiKeyResponse {
                    name: "ci-key".to_owned(),
                    authorized_user_id: Some(1),
                    scopes: vec![ApiKeyScope {
                        name: "universe-places".to_owned(),
                        operations: vec!["write".to_owned()],
                        user_ids: Vec::new(),
                        group_ids: Vec::new(),
                        universe_ids: vec!["999".to_owned()],
                    }],
                    enabled: true,
                    expired: false,
                    expiration_time_utc: None,
                }),
            },
        );

        assert!(report
            .blocking
            .iter()
            .any(|diagnostic| diagnostic.code == "api-key-missing-target-universe"));
    }

    #[test]
    fn warns_when_place_file_is_near_size_limit() {
        let temp = TempProject::new();
        temp.write_file("large.rbxlx", ROBLOX_PLACE_WARNING_BYTES as usize);

        let report = run_preflight_checks(
            temp.path(),
            &TargetConfig::Experience(empty_experience_target()),
            &OwnerConfig::Personal,
            &existing_graph(123),
            &desired_graph("large.rbxlx"),
            &PreflightAuthContext {
                open_cloud_api_key_present: true,
                open_cloud_key: Some(IntrospectApiKeyResponse {
                    name: "ci-key".to_owned(),
                    authorized_user_id: Some(1),
                    scopes: vec![ApiKeyScope {
                        name: "universe-places".to_owned(),
                        operations: vec!["write".to_owned()],
                        user_ids: Vec::new(),
                        group_ids: Vec::new(),
                        universe_ids: vec!["*".to_owned()],
                    }],
                    enabled: true,
                    expired: false,
                    expiration_time_utc: None,
                }),
            },
        );

        assert!(report
            .warnings
            .iter()
            .any(|diagnostic| diagnostic.code == "place-file-near-limit"));
    }

    #[test]
    fn blocks_group_friends_access() {
        let temp = TempProject::new();
        temp.write_file("game.rbxl", 128);

        let mut target = empty_experience_target();
        target.configuration = Some(crate::config::ExperienceTargetConfigurationConfig {
            playability: Some(PlayabilityTargetConfig::Friends),
            ..Default::default()
        });

        let report = run_preflight_checks(
            temp.path(),
            &TargetConfig::Experience(target),
            &OwnerConfig::Group(42),
            &existing_graph(123),
            &desired_graph("game.rbxl"),
            &PreflightAuthContext {
                open_cloud_api_key_present: true,
                open_cloud_key: Some(IntrospectApiKeyResponse {
                    name: "ci-key".to_owned(),
                    authorized_user_id: Some(1),
                    scopes: vec![ApiKeyScope {
                        name: "universe-places".to_owned(),
                        operations: vec!["write".to_owned()],
                        user_ids: Vec::new(),
                        group_ids: Vec::new(),
                        universe_ids: vec!["*".to_owned()],
                    }],
                    enabled: true,
                    expired: false,
                    expiration_time_utc: None,
                }),
            },
        );

        assert!(report
            .blocking
            .iter()
            .any(|diagnostic| diagnostic.code == "group-friends-access-unsupported"));
    }

    #[test]
    fn classifies_place_publish_permission_denied() {
        let error = map_roblox_api_error(
            OperationContext {
                action: OperationAction::Update,
                resource_id: Some("placeFile_start".to_owned()),
                resource_type: "PlaceFile".to_owned(),
                auth_model: "open-cloud".to_owned(),
                creator_target: Some("group:42".to_owned()),
                endpoint: Some("publish_place_version".to_owned()),
                file_path: Some("game.rbxl".to_owned()),
            },
            &RobloxApiError::Roblox {
                status_code: StatusCode::FORBIDDEN,
                reason: "Forbidden".to_owned(),
            },
        );

        assert_eq!(
            error.summary(),
            "Roblox denied the place publishing request."
        );
        assert!(error
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == "place-publish-permission-denied"));
    }

    #[test]
    fn classifies_group_experience_vague_server_failure() {
        let error = map_roblox_api_error(
            OperationContext {
                action: OperationAction::Create,
                resource_id: Some("experience_singleton".to_owned()),
                resource_type: "Experience".to_owned(),
                auth_model: "cookie-session".to_owned(),
                creator_target: Some("group:42".to_owned()),
                endpoint: Some("create_experience".to_owned()),
                file_path: None,
            },
            &RobloxApiError::Roblox {
                status_code: StatusCode::INTERNAL_SERVER_ERROR,
                reason: "Internal Server Error".to_owned(),
            },
        );

        assert!(error
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == "group-experience-create-vague-failure"));
    }
}

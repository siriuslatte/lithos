use serde::Deserialize;

use crate::models::AssetId;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntrospectApiKeyResponse {
    pub name: String,
    pub authorized_user_id: Option<AssetId>,
    #[serde(default)]
    pub scopes: Vec<ApiKeyScope>,
    pub enabled: bool,
    pub expired: bool,
    pub expiration_time_utc: Option<String>,
}

impl IntrospectApiKeyResponse {
    pub fn has_scope_operation(&self, scope_name: &str, operation: &str) -> bool {
        self.scopes
            .iter()
            .any(|scope| scope.matches(scope_name, operation))
    }

    pub fn allows_universe_operation(
        &self,
        scope_name: &str,
        operation: &str,
        universe_id: AssetId,
    ) -> bool {
        self.scopes.iter().any(|scope| {
            scope.matches(scope_name, operation)
                && scope
                    .universe_ids
                    .iter()
                    .any(|value| value == "*" || value == &universe_id.to_string())
        })
    }

    pub fn has_wildcard_universe_operation(&self, scope_name: &str, operation: &str) -> bool {
        self.scopes.iter().any(|scope| {
            scope.matches(scope_name, operation)
                && scope.universe_ids.iter().any(|value| value == "*")
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyScope {
    pub name: String,
    #[serde(default)]
    pub operations: Vec<String>,
    #[serde(default)]
    pub user_ids: Vec<String>,
    #[serde(default)]
    pub group_ids: Vec<String>,
    #[serde(default)]
    pub universe_ids: Vec<String>,
}

impl ApiKeyScope {
    pub fn matches(&self, scope_name: &str, operation: &str) -> bool {
        self.name == scope_name
            && self
                .operations
                .iter()
                .any(|value| value.eq_ignore_ascii_case(operation))
    }
}

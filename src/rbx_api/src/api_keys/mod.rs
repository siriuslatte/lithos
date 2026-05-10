pub mod models;

use serde_json::json;

use crate::{errors::RobloxApiResult, helpers::get_roblox_api_error_from_response, RobloxApi};

use self::models::IntrospectApiKeyResponse;

impl RobloxApi {
    pub async fn introspect_api_key(
        &self,
        api_key: &str,
    ) -> RobloxApiResult<IntrospectApiKeyResponse> {
        let response = self
            .client
            .post("https://apis.roblox.com/api-keys/v1/introspect")
            .json(&json!({ "apiKey": api_key }))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(response.json::<IntrospectApiKeyResponse>().await?)
        } else {
            Err(get_roblox_api_error_from_response(response).await)
        }
    }
}

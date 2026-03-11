use anyhow::{Context, Result, bail};
use reqwest::Client;
use serde::de::DeserializeOwned;
use serde_json::Value;

const API_URL: &str = "https://api.monday.com/v2";
const API_VERSION: &str = "2024-10";

pub struct MondayClient {
    http: Client,
    api_token: String,
}

impl MondayClient {
    pub fn new(api_token: String) -> Self {
        Self {
            http: Client::new(),
            api_token,
        }
    }

    pub async fn query<T: DeserializeOwned>(
        &self,
        query: &str,
        variables: Value,
    ) -> Result<T> {
        let body = serde_json::json!({
            "query": query,
            "variables": variables,
        });

        let resp = self
            .http
            .post(API_URL)
            .header("Authorization", &self.api_token)
            .header("API-Version", API_VERSION)
            .json(&body)
            .send()
            .await
            .context("failed to reach monday.com API")?;

        let status = resp.status();
        if status == 429 {
            bail!("monday.com rate limit exceeded (HTTP 429). Wait and retry.");
        }
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            bail!("monday.com API returned HTTP {status}: {text}");
        }

        let json: Value = resp.json().await.context("failed to parse API response")?;

        if let Some(errors) = json.get("errors") {
            if let Some(arr) = errors.as_array() {
                if !arr.is_empty() {
                    let msgs: Vec<String> = arr
                        .iter()
                        .filter_map(|e| e.get("message").and_then(|m| m.as_str()))
                        .map(String::from)
                        .collect();
                    bail!("monday.com API errors: {}", msgs.join("; "));
                }
            }
        }

        let data = json
            .get("data")
            .context("monday.com API response missing `data` field")?
            .clone();

        serde_json::from_value(data).context("failed to deserialize monday.com API response")
    }
}

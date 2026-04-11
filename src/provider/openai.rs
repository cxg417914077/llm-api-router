use super::{ChatCompletionsRequest, ChatCompletionsResponse, Provider};
use crate::config::ProviderConfig;
use crate::error::RouterError;
use async_trait::async_trait;
use reqwest::Client;

pub struct OpenAIProvider {
    config: ProviderConfig,
    client: Client,
}

impl OpenAIProvider {
    pub fn new(config: ProviderConfig) -> Result<Self, RouterError> {
        let client = Client::builder()
            .danger_accept_invalid_certs(!config.ssl_verify)
            .build()
            .map_err(RouterError::Network)?;

        Ok(Self { config, client })
    }
}

#[async_trait]
impl Provider for OpenAIProvider {
    fn name(&self) -> &str {
        &self.config.name
    }

    fn endpoint(&self) -> &str {
        &self.config.endpoint
    }

    fn priority(&self) -> u32 {
        self.config.priority
    }

    fn ssl_verify(&self) -> bool {
        self.config.ssl_verify
    }

    async fn chat_completions(
        &self,
        request: ChatCompletionsRequest,
    ) -> Result<ChatCompletionsResponse, RouterError> {
        let url = format!("{}/chat/completions", self.endpoint());

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        let status = response.status().as_u16();

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();

            // 检查特定错误类型
            if status == 429 {
                return Err(RouterError::RateLimit);
            }
            if body.contains("context_length") || body.contains("max_tokens") {
                return Err(RouterError::ContextLengthExceeded);
            }

            return Err(RouterError::HttpError { status, body });
        }

        // 先获取原始文本，方便调试
        let body = response.text().await.unwrap_or_default();
        tracing::info!("下游响应体：{}", body);

        // 尝试解析 JSON
        let response: ChatCompletionsResponse = serde_json::from_str(&body)
            .map_err(|e| {
                tracing::error!("JSON 解析失败：{}, 响应体：{}", e, body);
                RouterError::HttpError {
                    status,
                    body: format!("JSON 解析失败：{}, 响应体：{}", e, body),
                }
            })?;
        Ok(response)
    }
}

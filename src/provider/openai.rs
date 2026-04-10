use super::{ChatCompletionsRequest, ChatCompletionsResponse, Provider};
use crate::config::ProviderConfig;
use crate::error::RouterError;
use async_trait::async_trait;

pub struct OpenAIProvider {
    config: ProviderConfig,
}

impl OpenAIProvider {
    pub fn new(config: ProviderConfig) -> Self {
        Self { config }
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

    async fn chat_completions(
        &self,
        request: ChatCompletionsRequest,
        client: &reqwest::Client,
    ) -> Result<ChatCompletionsResponse, RouterError> {
        let url = format!("{}/chat/completions", self.endpoint());

        let response = client
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

        let response: ChatCompletionsResponse = response.json().await?;
        Ok(response)
    }
}

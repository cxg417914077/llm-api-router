use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Chat Completions 请求（OpenAI 兼容格式）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionsRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// Chat Completions 响应（OpenAI 兼容格式）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionsResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Usage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    pub index: u32,
    pub message: ChatMessage,
    pub finish_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Provider Trait - 所有 LLM 提供商必须实现此接口
#[async_trait]
pub trait Provider: Send + Sync {
    fn name(&self) -> &str;
    fn endpoint(&self) -> &str;
    #[allow(dead_code)]
    fn priority(&self) -> u32;
    #[allow(dead_code)]
    fn ssl_verify(&self) -> bool;

    async fn chat_completions(
        &self,
        request: ChatCompletionsRequest,
        client: &reqwest::Client,
    ) -> Result<ChatCompletionsResponse, crate::error::RouterError>;
}

pub mod openai;

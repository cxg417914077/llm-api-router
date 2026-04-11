use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// Chat Completions 请求（OpenAI 兼容格式）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionsRequest {
    // 必需字段
    pub model: String,
    pub messages: Vec<ChatMessage>,

    // 核心参数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,

    // 采样参数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,

    // 停止序列
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<StopSequence>,

    // 生成数量
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<u32>,

    // Logit 偏差
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logit_bias: Option<std::collections::HashMap<String, f32>>,

    // 用户标识
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,

    // 响应格式
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,

    // 随机种子
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,

    // 服务层级
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,

    // 并行工具调用
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,

    /// 工具列表（function call / tools）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,

    /// 工具选择策略
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,

    /// 是否返回 logprobs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<bool>,

    /// top_logprobs 数量
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<u32>,
}

/// 停止序列（支持字符串或字符串数组）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StopSequence {
    String(String),
    Array(Vec<String>),
}

/// 响应格式（JSON mode / Text mode）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseFormat {
    #[serde(rename = "type")]
    pub response_type: String, // "text", "json_object", "json_schema"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_schema: Option<JsonSchema>,
}

/// JSON Schema 定义（用于 json_schema 响应格式）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonSchema {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub schema: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
}

/// 工具定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionDefinition,
}

/// 函数定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// 工具选择策略
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolChoice {
    /// "none", "auto", "required"
    String(String),
    /// 指定特定函数
    Object {
        #[serde(rename = "type")]
        tool_type: String,
        function: FunctionReference,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionReference {
    pub name: String,
}

/// 工具调用（用于助手消息）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionCall,
}

/// 函数调用（工具调用的具体参数）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    /// 参数字符串（JSON 格式）
    pub arguments: String,
}

/// 流式响应中的工具调用 delta
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallDelta {
    pub index: u32,
    #[serde(rename = "type")]
    pub tool_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub function: FunctionCallDelta,
}

/// 流式响应中的函数调用 delta
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCallDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}

/// 流式响应块（OpenAI 兼容格式）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionsChunk {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<ChunkChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
    /// 系统指纹（用于检测后端变化）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,
    /// 服务层级
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkChoice {
    pub index: u32,
    pub delta: ChatMessageDelta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<FinishReason>,
    /// 日志概率（如果请求了 logprobs）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<ChatCompletionLogprobs>,
}

/// 流式响应中的 delta 字段（所有字段都可以为 null）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessageDelta {
    #[serde(default)]
    pub role: Option<String>,
    /// 内容（流式字符串）
    #[serde(default)]
    pub content: Option<String>,
    /// 工具调用列表（流式）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallDelta>>,
    /// 拒绝内容（流式）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
}

/// 消息内容（支持字符串或多模态部分）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    String(String),
    Parts(Vec<ContentPart>),
}

impl Default for MessageContent {
    fn default() -> Self {
        MessageContent::String(String::new())
    }
}

/// 内容部分（用于多模态消息）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentPart {
    #[serde(rename = "type")]
    pub part_type: String, // "text", "image_url", "input_audio"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<ImageUrl>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_audio: Option<InputAudio>,
}

/// 图片 URL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrl {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>, // "auto", "low", "high"
}

/// 输入音频
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputAudio {
    pub data: String, // base64 编码
    pub format: String, // "wav", "mp3"
}

/// 非流式响应中的完整消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    /// 消息内容（可以是字符串或 ContentPart 数组）
    #[serde(default)]
    pub content: MessageContent,
    /// 工具调用列表（仅助手消息）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    /// 工具响应 ID（仅工具消息）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// 消息名称（可选，用于标识特定用户或助手）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// 拒绝内容（当模型拒绝回答时）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
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
    /// 系统指纹（用于检测后端变化）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,
    /// 服务层级
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,
}

/// 完成原因
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FinishReason {
    #[serde(rename = "stop")]
    Stop,
    #[serde(rename = "length")]
    Length,
    #[serde(rename = "tool_calls")]
    ToolCalls,
    #[serde(rename = "content_filter")]
    ContentFilter,
    #[serde(rename = "function_call")]
    FunctionCall,
    #[serde(untagged)]
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    pub index: u32,
    pub message: ChatMessage,
    pub finish_reason: FinishReason,
    /// 日志概率（如果请求了 logprobs）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<ChatCompletionLogprobs>,
}

/// 日志概率
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionLogprobs {
    pub content: Vec<LogprobContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<Vec<LogprobContent>>,
}

/// 日志概率内容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogprobContent {
    pub token: String,
    pub logprob: f32,
    pub bytes: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<Vec<TopLogprob>>,
}

/// Top 日志概率
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopLogprob {
    pub token: String,
    pub logprob: f32,
    pub bytes: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    /// 提示词缓存详情
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_tokens_details: Option<PromptTokensDetails>,
    /// 完成 tokens 详情
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_tokens_details: Option<CompletionTokensDetails>,
}

/// 提示词 tokens 详情
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTokensDetails {
    #[serde(default)]
    pub cached_tokens: u32,
}

/// 完成 tokens 详情
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionTokensDetails {
    #[serde(default)]
    pub accepted_prediction_tokens: u32,
    #[serde(default)]
    pub rejected_prediction_tokens: u32,
    #[serde(default)]
    pub reasoning_tokens: u32,
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
    ) -> Result<ChatCompletionsResponse, crate::error::RouterError>;

    /// 流式 chat completions 请求
    async fn chat_completions_stream(
        &self,
        request: ChatCompletionsRequest,
    ) -> Result<
        mpsc::Receiver<Result<ChatCompletionsChunk, crate::error::RouterError>>,
        crate::error::RouterError,
    > {
        // 默认实现：使用非流式请求，然后转换为流式
        let response = self.chat_completions(request).await?;
        let (tx, rx) = mpsc::channel(1);

        // 将完整响应转换为单个 chunk
        let chunk = ChatCompletionsChunk {
            id: response.id,
            object: response.object,
            created: response.created,
            model: response.model,
            choices: response
                .choices
                .into_iter()
                .map(|c| ChunkChoice {
                    index: c.index,
                    delta: ChatMessageDelta {
                        role: Some(c.message.role),
                        content: match &c.message.content {
                            MessageContent::String(s) => Some(s.clone()),
                            MessageContent::Parts(parts) => {
                                // 多模态内容转换为纯文本
                                Some(parts.iter().filter_map(|p| p.text.clone()).collect::<Vec<_>>().join(""))
                            }
                        },
                        tool_calls: None,
                        refusal: c.message.refusal.clone(),
                    },
                    finish_reason: Some(c.finish_reason.clone()),
                    logprobs: c.logprobs.clone(),
                })
                .collect(),
            usage: Some(response.usage),
            system_fingerprint: response.system_fingerprint.clone(),
            service_tier: response.service_tier.clone(),
        };

        tx.send(Ok(chunk)).await.ok();
        Ok(rx)
    }
}

// ==================== Models API 数据结构 ====================

/// 模型列表响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelsListResponse {
    pub object: String,
    pub data: Vec<ModelInfo>,
}

/// 单个模型信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub owned_by: String,
    /// 模型类型
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_type: Option<String>,
    /// 根模型 ID（用于派生模型）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    /// 父模型 ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// 权限信息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission: Option<Vec<ModelPermission>>,
    /// 上下文窗口大小
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_window: Option<u32>,
    /// 支持的训练类型
    #[serde(skip_serializing_if = "Option::is_none")]
    pub training_type: Option<Vec<String>>,
    /// 能力描述
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<ModelCapabilities>,
}

/// 模型权限
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPermission {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub allow_create_engine: bool,
    pub allow_sampling: bool,
    pub allow_logprobs: bool,
    pub allow_search_indices: bool,
    pub allow_view: bool,
    pub allow_fine_tuning: bool,
    pub organization: String,
    pub group: Option<String>,
    pub is_blocking: bool,
}

/// 模型能力
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_chat: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_text: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vision_image: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_call: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<bool>,
}

pub mod openai;

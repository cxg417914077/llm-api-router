use super::{ChatCompletionsChunk, ChatCompletionsRequest, ChatCompletionsResponse, Provider};
use crate::config::ProviderConfig;
use crate::error::RouterError;
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use tokio::sync::mpsc;

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

        // 强制设置 stream: false，确保下游返回完整的 JSON 响应（而非 SSE 流式格式）
        let mut downstream_request = request.clone();
        downstream_request.stream = Some(false);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&downstream_request)
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
        let response: ChatCompletionsResponse = serde_json::from_str(&body).map_err(|e| {
            tracing::error!("JSON 解析失败：{}, 响应体：{}", e, body);
            RouterError::HttpError {
                status,
                body: format!("JSON 解析失败：{}, 响应体：{}", e, body),
            }
        })?;
        Ok(response)
    }

    async fn chat_completions_stream(
        &self,
        request: ChatCompletionsRequest,
    ) -> Result<mpsc::Receiver<Result<ChatCompletionsChunk, RouterError>>, RouterError> {
        let url = format!("{}/chat/completions", self.endpoint());

        // 强制设置 stream: true，请求 SSE 流式响应
        let mut downstream_request = request.clone();
        downstream_request.stream = Some(true);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&downstream_request)
            .send()
            .await?;

        let status = response.status();

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();

            if status.as_u16() == 429 {
                return Err(RouterError::RateLimit);
            }
            if body.contains("context_length") || body.contains("max_tokens") {
                return Err(RouterError::ContextLengthExceeded);
            }

            return Err(RouterError::HttpError {
                status: status.as_u16(),
                body,
            });
        }

        // 创建通道用于发送流式块
        let (tx, rx) = mpsc::channel(100);

        // 启动异步任务读取 SSE 流
        let stream = response.bytes_stream();
        let mut stream = stream.fuse();
        let mut buffer = String::new();

        tokio::spawn(async move {
            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(bytes) => {
                        let text = String::from_utf8_lossy(&bytes);
                        buffer.push_str(&text);

                        // 按行处理 SSE 数据
                        let lines: Vec<&str> = buffer.lines().collect();
                        let mut new_buffer = String::new();

                        for line in lines {
                            let line = line.trim();

                            // 跳过空行
                            if line.is_empty() {
                                continue;
                            }

                            // 检查是否是 SSE data 行
                            if let Some(data) = line.strip_prefix("data: ") {
                                // 检查是否是结束标记
                                if data == "[DONE]" {
                                    tracing::debug!("SSE stream ended");
                                    break;
                                }

                                // 尝试解析 JSON chunk
                                match serde_json::from_str::<ChatCompletionsChunk>(data) {
                                    Ok(chunk) => {
                                        if tx.send(Ok(chunk)).await.is_err() {
                                            // 接收端已关闭，停止发送
                                            break;
                                        }
                                    }
                                    Err(e) => {
                                        tracing::error!(
                                            "解析 SSE chunk 失败：{}, data: {}",
                                            e,
                                            data
                                        );
                                        // 继续处理下一个 chunk
                                    }
                                }
                            } else if let Some(data) = line.strip_prefix("data:") {
                                // 处理没有空格的情况 "data:xxx"
                                if data == "[DONE]" {
                                    break;
                                }
                                if let Ok(chunk) =
                                    serde_json::from_str::<ChatCompletionsChunk>(data)
                                {
                                    if tx.send(Ok(chunk)).await.is_err() {
                                        break;
                                    }
                                }
                            } else {
                                // 非 data 行，保留到缓冲区（可能是不完整的行）
                                new_buffer.push_str(line);
                                new_buffer.push('\n');
                            }
                        }

                        buffer = new_buffer;
                    }
                    Err(e) => {
                        tracing::error!("SSE 流读取错误：{:?}", e);
                        let err = RouterError::Network(e);
                        let _ = tx.send(Err(err)).await;
                        break;
                    }
                }
            }

            tracing::debug!("SSE 流读取完成");
        });

        Ok(rx)
    }
}

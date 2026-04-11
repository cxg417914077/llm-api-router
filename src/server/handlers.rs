use axum::{
    extract::State,
    middleware,
    response::sse::{Event, KeepAlive, Sse},
    routing::{get, post},
    Json, Router,
};
use futures_util::Stream;
use pin_project_lite::pin_project;
use std::convert::Infallible;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::sync::mpsc;

use crate::config::Config;
use crate::error::{Result, RouterError};
use crate::provider::{ChatCompletionsRequest, ChatCompletionsResponse, Provider};
use crate::routing::RoutingEngine;
use crate::server::auth::auth_middleware;

pub struct AppState {
    pub config: Config,
    pub routing_engine: RoutingEngine,
    pub providers: Vec<Arc<dyn Provider>>,
}

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/v1/chat/completions", post(chat_completions_handler))
        .route("/v1/models", get(models_list_handler))
        .route("/v1/models/:model_id", get(models_get_handler))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .with_state(state)
}

#[axum::debug_handler]
async fn chat_completions_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ChatCompletionsRequest>,
) -> Result<HandlerResponse> {
    // 从请求的 model 字段解析组名
    let group_name = &request.model;
    tracing::info!(
        "🔀 收到请求：组名='{}', 原始 model='{}', stream={:?}",
        group_name,
        request.model,
        request.stream
    );

    // 根据组名获取配置
    let mut group_config = state
        .config
        .groups
        .get(group_name)
        .ok_or_else(|| {
            crate::error::RouterError::Config(format!(
                "Group '{}' not found in configuration",
                group_name
            ))
        })?
        .clone();

    // 按优先级排序 providers
    group_config.providers.sort_by_key(|p| p.priority);

    tracing::info!(
        "📋 组配置：共 {} 个 providers (按优先级排序)",
        group_config.providers.len()
    );
    for (idx, p) in group_config.providers.iter().enumerate() {
        tracing::info!(
            "   Provider[{}]: name='{}', priority={}, models={:?}",
            idx,
            p.name,
            p.priority,
            p.models
        );
    }

    // 检查是否是流式请求
    let is_streaming = request.stream.unwrap_or(false);

    if is_streaming {
        handle_streaming_request(state, group_config, request).await
    } else {
        handle_non_streaming_request(state, group_config, request).await
    }
}

/// 处理非流式请求（保持原有逻辑）
async fn handle_non_streaming_request(
    state: Arc<AppState>,
    group_config: crate::config::GroupConfig,
    request: ChatCompletionsRequest,
) -> Result<HandlerResponse> {
    let group_name = request.model.clone();
    let mut current_provider_idx: usize = 0;
    let mut attempt_count = 0;

    loop {
        let provider = match group_config.providers.get(current_provider_idx) {
            Some(p) => p,
            None => {
                tracing::error!("❌ 所有 Provider 都已尝试，请求失败");
                return Err(crate::error::RouterError::AllProvidersFailed);
            }
        };

        if provider.models.is_empty() {
            let model_for_downstream = request.model.clone();
            tracing::info!(
                "⚠️  Provider '{}' 未配置 models，使用原始 model='{}'",
                provider.name,
                model_for_downstream
            );

            attempt_count += 1;
            tracing::info!(
                "🚀 尝试请求 #{}: provider='{}', endpoint='{}', model='{}'",
                attempt_count,
                provider.name,
                provider.endpoint,
                model_for_downstream
            );

            let provider_impl = state
                .providers
                .iter()
                .find(|p| p.name() == provider.name)
                .ok_or_else(|| {
                    crate::error::RouterError::ProviderNotFound(provider.name.clone())
                })?;

            let mut downstream_request = request.clone();
            downstream_request.model = model_for_downstream.clone();

            match provider_impl.chat_completions(downstream_request).await {
                Ok(response) => {
                    tracing::info!(
                        "✅ 请求成功：provider='{}', model='{}'",
                        provider.name,
                        model_for_downstream
                    );
                    state.routing_engine.health_tracker().record_success(
                        &group_name,
                        &provider.name,
                        &model_for_downstream,
                    );
                    return Ok(HandlerResponse::Json(Json(response)));
                }
                Err(e) => {
                    tracing::warn!(
                        "❌ Provider {} (model: {}) 失败：{:?}，切换到下一个 provider",
                        provider.name,
                        model_for_downstream,
                        e
                    );
                    state.routing_engine.health_tracker().record_failure(
                        &group_name,
                        &provider.name,
                        &model_for_downstream,
                    );
                    current_provider_idx += 1;
                    continue;
                }
            }
        }

        let mut found_healthy_model = false;
        let mut model_for_downstream: Option<String> = None;

        for i in 0..provider.models.len() {
            let model_idx = i % provider.models.len();
            let model = &provider.models[model_idx];

            if state
                .routing_engine
                .health_tracker()
                .is_healthy(&group_name, &provider.name, model)
            {
                model_for_downstream = Some(model.clone());
                found_healthy_model = true;
                tracing::info!(
                    "🎯 Provider '{}' 选择 model='{}' (models[{}], 健康检查通过)",
                    provider.name,
                    model,
                    model_idx
                );
                break;
            } else {
                tracing::info!(
                    "⏭️  Provider '{}' 跳过不健康的 model='{}' (models[{}])",
                    provider.name,
                    model,
                    model_idx
                );
            }
        }

        if !found_healthy_model {
            tracing::info!(
                "⚠️  Provider '{}' 的所有 models 都不健康，切换到下一个 provider",
                provider.name
            );
            current_provider_idx += 1;
            continue;
        }

        let model_for_downstream = model_for_downstream.unwrap();

        attempt_count += 1;
        tracing::info!(
            "🚀 尝试请求 #{}: provider='{}', endpoint='{}', model='{}'",
            attempt_count,
            provider.name,
            provider.endpoint,
            model_for_downstream
        );

        let provider_impl = state
            .providers
            .iter()
            .find(|p| p.name() == provider.name)
            .ok_or_else(|| crate::error::RouterError::ProviderNotFound(provider.name.clone()))?;

        let mut downstream_request = request.clone();
        downstream_request.model = model_for_downstream.clone();

        match provider_impl.chat_completions(downstream_request).await {
            Ok(response) => {
                tracing::info!(
                    "✅ 请求成功：provider='{}', model='{}'",
                    provider.name,
                    model_for_downstream
                );
                state.routing_engine.health_tracker().record_success(
                    &group_name,
                    &provider.name,
                    &model_for_downstream,
                );
                return Ok(HandlerResponse::Json(Json(response)));
            }
            Err(e) => {
                tracing::warn!(
                    "❌ Provider {} (model: {}) 失败：{:?}，尝试下一个 model...",
                    provider.name,
                    model_for_downstream,
                    e
                );
                state.routing_engine.health_tracker().record_failure(
                    &group_name,
                    &provider.name,
                    &model_for_downstream,
                );
            }
        }
    }
}

/// 处理流式请求
async fn handle_streaming_request(
    state: Arc<AppState>,
    group_config: crate::config::GroupConfig,
    request: ChatCompletionsRequest,
) -> Result<HandlerResponse> {
    let group_name = request.model.clone();
    let mut current_provider_idx: usize = 0;

    loop {
        let provider = match group_config.providers.get(current_provider_idx) {
            Some(p) => p,
            None => {
                tracing::error!("❌ 所有 Provider 都已尝试，流式请求失败");
                return Err(crate::error::RouterError::AllProvidersFailed);
            }
        };

        // 获取要使用的 model
        let model_for_downstream = if provider.models.is_empty() {
            request.model.clone()
        } else {
            // 找到第一个健康的 model
            let mut found = false;
            let mut selected_model = request.model.clone();

            for i in 0..provider.models.len() {
                let model_idx = i % provider.models.len();
                let model = &provider.models[model_idx];

                if state.routing_engine.health_tracker().is_healthy(
                    &group_name,
                    &provider.name,
                    model,
                ) {
                    selected_model = model.clone();
                    found = true;
                    tracing::info!(
                        "🎯 Provider '{}' 选择 model='{}' (models[{}], 健康检查通过)",
                        provider.name,
                        model,
                        model_idx
                    );
                    break;
                } else {
                    tracing::info!(
                        "⏭️  Provider '{}' 跳过不健康的 model='{}' (models[{}])",
                        provider.name,
                        model,
                        model_idx
                    );
                }
            }

            if !found {
                tracing::info!(
                    "⚠️  Provider '{}' 的所有 models 都不健康，切换到下一个 provider",
                    provider.name
                );
                current_provider_idx += 1;
                continue;
            }

            selected_model
        };

        tracing::info!(
            "🚀 流式尝试 #{}: provider='{}', endpoint='{}', model='{}'",
            current_provider_idx + 1,
            provider.name,
            provider.endpoint,
            model_for_downstream
        );

        let provider_impl = state
            .providers
            .iter()
            .find(|p| p.name() == provider.name)
            .ok_or_else(|| crate::error::RouterError::ProviderNotFound(provider.name.clone()))?;

        let mut downstream_request = request.clone();
        downstream_request.model = model_for_downstream.clone();

        // 调用流式接口
        match provider_impl
            .chat_completions_stream(downstream_request)
            .await
        {
            Ok(rx) => {
                tracing::info!(
                    "✅ 流式请求启动成功：provider='{}', model='{}'",
                    provider.name,
                    model_for_downstream
                );
                state.routing_engine.health_tracker().record_success(
                    &group_name,
                    &provider.name,
                    &model_for_downstream,
                );

                // 将接收通道转换为 SSE 流
                let health_tracker = state.routing_engine.health_tracker().clone();
                let stream = StreamWrapper::new(
                    rx,
                    Arc::new(health_tracker),
                    group_name.clone(),
                    provider.name.to_string(),
                    model_for_downstream.clone(),
                );

                let sse = Sse::new(stream).keep_alive(
                    KeepAlive::new()
                        .interval(std::time::Duration::from_secs(15))
                        .text("ping"),
                );

                return Ok(HandlerResponse::Sse(sse));
            }
            Err(e) => {
                tracing::warn!(
                    "❌ Provider {} (model: {}) 流式请求失败：{:?}，切换到下一个 provider",
                    provider.name,
                    model_for_downstream,
                    e
                );
                state.routing_engine.health_tracker().record_failure(
                    &group_name,
                    &provider.name,
                    &model_for_downstream,
                );
                current_provider_idx += 1;
                continue;
            }
        }
    }
}

pin_project! {
    pub struct StreamWrapper {
        rx: mpsc::Receiver<std::result::Result<crate::provider::ChatCompletionsChunk, RouterError>>,
        health_tracker: Arc<crate::health::HealthTracker>,
        group_name: String,
        provider_name: String,
        model: String,
    }
}

impl StreamWrapper {
    pub fn new(
        rx: mpsc::Receiver<std::result::Result<crate::provider::ChatCompletionsChunk, RouterError>>,
        health_tracker: Arc<crate::health::HealthTracker>,
        group_name: String,
        provider_name: String,
        model: String,
    ) -> Self {
        Self {
            rx,
            health_tracker,
            group_name,
            provider_name,
            model,
        }
    }
}

impl Stream for StreamWrapper {
    type Item = std::result::Result<Event, Infallible>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.as_mut().project();
        let poll = this.rx.poll_recv(cx);

        match poll {
            Poll::Ready(Some(result)) => {
                match result {
                    Ok(chunk) => {
                        // 记录成功
                        this.health_tracker.record_success(
                            this.group_name,
                            this.provider_name,
                            this.model,
                        );

                        // 将 chunk 序列化为 JSON 字符串
                        let data = serde_json::to_string(&chunk).unwrap_or_else(|e| {
                            tracing::error!("序列化 chunk 失败：{}", e);
                            String::from("{}")
                        });

                        Poll::Ready(Some(Ok(Event::default().data(data))))
                    }
                    Err(e) => {
                        tracing::warn!(
                            "❌ 流式块错误：provider={}, model={}, error={:?}",
                            this.provider_name,
                            this.model,
                            e
                        );
                        this.health_tracker.record_failure(
                            this.group_name,
                            this.provider_name,
                            this.model,
                        );

                        // 发送错误事件
                        let error_data = serde_json::json!({
                            "error": {
                                "message": format!("{}", e),
                                "type": "router_error"
                            }
                        });
                        let data = serde_json::to_string(&error_data).unwrap_or_default();
                        Poll::Ready(Some(Ok(Event::default().data(data))))
                    }
                }
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Handler 响应类型（支持 JSON 和 SSE）
pub enum HandlerResponse {
    Json(Json<ChatCompletionsResponse>),
    Sse(Sse<StreamWrapper>),
}

impl axum::response::IntoResponse for HandlerResponse {
    fn into_response(self) -> axum::response::Response {
        match self {
            HandlerResponse::Json(json) => json.into_response(),
            HandlerResponse::Sse(sse) => sse.into_response(),
        }
    }
}

// ==================== Models API Handlers ====================

/// GET /v1/models - 获取模型列表
#[axum::debug_handler]
async fn models_list_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<crate::provider::ModelsListResponse>> {
    tracing::info!("📋 获取模型列表");

    let mut models = Vec::new();
    let now = std::time::UNIX_EPOCH
        .elapsed()
        .unwrap()
        .as_secs();

    // 从所有组中收集模型信息
    for (group_name, group_config) in &state.config.groups {
        for provider_config in &group_config.providers {
            // 如果 provider 配置了 models，使用配置的 models
            if !provider_config.models.is_empty() {
                for model in &provider_config.models {
                    models.push(crate::provider::ModelInfo {
                        id: model.clone(),
                        object: "model".to_string(),
                        created: now,
                        owned_by: provider_config.name.clone(),
                        model_type: Some("chat".to_string()),
                        root: Some(model.clone()),
                        parent: None,
                        permission: None,
                        context_window: None,
                        training_type: None,
                        capabilities: None,
                    });
                }
            } else {
                // 如果没有配置 models，使用组名作为模型 ID
                models.push(crate::provider::ModelInfo {
                    id: group_name.clone(),
                    object: "model".to_string(),
                    created: now,
                    owned_by: provider_config.name.clone(),
                    model_type: Some("chat".to_string()),
                    root: Some(group_name.clone()),
                    parent: None,
                    permission: None,
                    context_window: None,
                    training_type: None,
                    capabilities: None,
                });
            }
        }
    }

    // 去重
    models.sort_by(|a, b| a.id.cmp(&b.id));
    models.dedup_by(|a, b| a.id == b.id);

    Ok(Json(crate::provider::ModelsListResponse {
        object: "list".to_string(),
        data: models,
    }))
}

/// GET /v1/models/:model_id - 获取单个模型详情
#[axum::debug_handler]
async fn models_get_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(model_id): axum::extract::Path<String>,
) -> Result<Json<crate::provider::ModelInfo>> {
    tracing::info!("🔍 获取模型详情：{}", model_id);

    let now = std::time::UNIX_EPOCH
        .elapsed()
        .unwrap()
        .as_secs();

    // 查找模型
    for (group_name, group_config) in &state.config.groups {
        for provider_config in &group_config.providers {
            // 检查是否匹配组名（别名）
            if group_name == &model_id {
                return Ok(Json(crate::provider::ModelInfo {
                    id: model_id.clone(),
                    object: "model".to_string(),
                    created: now,
                    owned_by: provider_config.name.clone(),
                    model_type: Some("chat".to_string()),
                    root: Some(group_name.clone()),
                    parent: None,
                    permission: None,
                    context_window: None,
                    training_type: None,
                    capabilities: None,
                }));
            }

            // 检查是否匹配配置的 models
            for model in &provider_config.models {
                if model == &model_id {
                    return Ok(Json(crate::provider::ModelInfo {
                        id: model_id.clone(),
                        object: "model".to_string(),
                        created: now,
                        owned_by: provider_config.name.clone(),
                        model_type: Some("chat".to_string()),
                        root: Some(model.clone()),
                        parent: None,
                        permission: None,
                        context_window: None,
                        training_type: None,
                        capabilities: None,
                    }));
                }
            }
        }
    }

    Err(RouterError::NotFound(format!(
        "Model '{}' not found",
        model_id
    )))
}

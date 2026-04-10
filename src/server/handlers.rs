use axum::{
    extract::State,
    middleware,
    routing::post,
    Json, Router,
};
use std::sync::Arc;

use crate::config::Config;
use crate::error::Result;
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
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
        .with_state(state)
}

#[axum::debug_handler]
async fn chat_completions_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ChatCompletionsRequest>,
) -> Result<Json<ChatCompletionsResponse>> {
    // 从请求的 model 字段解析组名
    let group_name = &request.model;
    tracing::info!("🔀 收到请求：组名='{}', 原始 model='{}'", group_name, request.model);

    // 根据组名获取配置
    let mut group_config = state
        .config
        .groups
        .get(group_name)
        .ok_or_else(|| crate::error::RouterError::Config(
            format!("Group '{}' not found in configuration", group_name)
        ))?
        .clone();

    // 按优先级排序 providers
    group_config.providers.sort_by_key(|p| p.priority);

    tracing::info!("📋 组配置：共 {} 个 providers (按优先级排序)", group_config.providers.len());
    for (idx, p) in group_config.providers.iter().enumerate() {
        tracing::info!("   Provider[{}]: name='{}', priority={}, models={:?}", idx, p.name, p.priority, p.models);
    }

    let mut current_provider_idx: usize = 0;  // 当前尝试的 provider 索引
    let mut attempt_count = 0;

    // 双层循环：外层遍历 provider，内层遍历 models
    loop {
        // 获取当前 provider
        let provider = match group_config.providers.get(current_provider_idx) {
            Some(p) => p,
            None => {
                tracing::error!("❌ 所有 Provider 都已尝试，请求失败");
                return Err(crate::error::RouterError::AllProvidersFailed);
            }
        };

        // 检查 models 列表是否已用完
        if provider.models.is_empty() {
            // 没有配置 models，使用原始 model
            let model_for_downstream = request.model.clone();
            tracing::info!("⚠️  Provider '{}' 未配置 models，使用原始 model='{}'", provider.name, model_for_downstream);

            attempt_count += 1;
            tracing::info!("🚀 尝试请求 #{}: provider='{}', endpoint='{}', model='{}'",
                attempt_count, provider.name, provider.endpoint, model_for_downstream);

            let provider_impl = state
                .providers
                .iter()
                .find(|p| p.name() == provider.name)
                .ok_or_else(|| crate::error::RouterError::ProviderNotFound(provider.name.clone()))?;

            let mut downstream_request = request.clone();
            downstream_request.model = model_for_downstream.clone();

            match provider_impl.chat_completions(downstream_request).await {
                Ok(response) => {
                    tracing::info!("✅ 请求成功：provider='{}', model='{}'", provider.name, model_for_downstream);
                    state.routing_engine.health_tracker().record_success(group_name, &provider.name, &model_for_downstream);
                    return Ok(Json(response));
                }
                Err(e) => {
                    tracing::warn!("❌ Provider {} (model: {}) 失败：{:?}，切换到下一个 provider", provider.name, model_for_downstream, e);
                    state.routing_engine.health_tracker().record_failure(group_name, &provider.name, &model_for_downstream);
                    current_provider_idx += 1;
                    continue;
                }
            }
        }

        // 从当前索引开始，找到下一个健康的 model
        let mut found_healthy_model = false;
        let mut model_for_downstream: Option<String> = None;

        for i in 0..provider.models.len() {
            let model_idx = (0 + i) % provider.models.len();  // 循环遍历
            let model = &provider.models[model_idx];

            // 检查这个 model 是否健康
            if state.routing_engine.health_tracker().is_healthy(group_name, &provider.name, model) {
                model_for_downstream = Some(model.clone());
                found_healthy_model = true;
                tracing::info!("🎯 Provider '{}' 选择 model='{}' (models[{}], 健康检查通过)", provider.name, model, model_idx);
                break;
            } else {
                tracing::info!("⏭️  Provider '{}' 跳过不健康的 model='{}' (models[{}])", provider.name, model, model_idx);
            }
        }

        // 如果所有 models 都不健康，切换到下一个 provider
        if !found_healthy_model {
            tracing::info!("⚠️  Provider '{}' 的所有 models 都不健康，切换到下一个 provider", provider.name);
            current_provider_idx += 1;
            continue;
        }

        let model_for_downstream = model_for_downstream.unwrap();

        attempt_count += 1;
        tracing::info!("🚀 尝试请求 #{}: provider='{}', endpoint='{}', model='{}'",
            attempt_count, provider.name, provider.endpoint, model_for_downstream);

        // 找到对应的 Provider 实现
        let provider_impl = state
            .providers
            .iter()
            .find(|p| p.name() == provider.name)
            .ok_or_else(|| crate::error::RouterError::ProviderNotFound(provider.name.clone()))?;

        // 创建新的请求，使用 provider 配置的模型
        let mut downstream_request = request.clone();
        downstream_request.model = model_for_downstream.clone();

        match provider_impl.chat_completions(downstream_request).await {
            Ok(response) => {
                tracing::info!("✅ 请求成功：provider='{}', model='{}'", provider.name, model_for_downstream);
                state.routing_engine.health_tracker().record_success(group_name, &provider.name, &model_for_downstream);
                return Ok(Json(response));
            }
            Err(e) => {
                tracing::warn!("❌ Provider {} (model: {}) 失败：{:?}，尝试下一个 model...", provider.name, model_for_downstream, e);
                state.routing_engine.health_tracker().record_failure(group_name, &provider.name, &model_for_downstream);
                // 继续循环，下一个健康的 model 会被选中
            }
        }
    }
}

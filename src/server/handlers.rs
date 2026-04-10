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
    pub http_client: reqwest::Client,
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

    // 根据组名获取配置
    let group_config = state
        .config
        .groups
        .get(group_name)
        .ok_or_else(|| crate::error::RouterError::Config(
            format!("Group '{}' not found in configuration", group_name)
        ))?;

    let mut last_attempted: Option<String> = None;

    // 尝试每个 provider 直到成功
    loop {
        let provider = state
            .routing_engine
            .select_provider(group_name, &group_config.providers, last_attempted.as_deref());

        let provider = match provider {
            Some(p) => p,
            None => {
                return Err(crate::error::RouterError::AllProvidersFailed);
            }
        };

        last_attempted = Some(provider.name.clone());

        // 找到对应的 Provider 实现
        let provider_impl = state
            .providers
            .iter()
            .find(|p| p.name() == provider.name)
            .ok_or_else(|| crate::error::RouterError::ProviderNotFound(provider.name.clone()))?;

        match provider_impl.chat_completions(request.clone(), &state.http_client).await {
            Ok(response) => {
                state.routing_engine.health_tracker().record_success(group_name, &provider.name);
                return Ok(Json(response));
            }
            Err(e) => {
                tracing::warn!("Provider {} failed: {:?}, trying next...", provider.name, e);
                state.routing_engine.health_tracker().record_failure(group_name, &provider.name);
                // 继续尝试下一个 provider
            }
        }
    }
}

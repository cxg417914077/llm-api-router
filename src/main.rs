mod config;
mod error;
mod health;
mod routing;
mod provider;
mod server;

use std::sync::Arc;
use tracing_subscriber::EnvFilter;

use config::Config;
use health::HealthTracker;
use routing::RoutingEngine;
use server::{create_router, AppState};
use provider::{Provider, openai::OpenAIProvider};
use error::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // 加载配置
    let config = Config::load()?;
    tracing::info!("Loaded config from config.yaml");

    // 获取默认组（第一个组）
    let default_group = config
        .groups
        .values()
        .next()
        .ok_or_else(|| crate::error::RouterError::Config("No provider groups configured".to_string()))?;

    // 创建健康追踪器
    let health_tracker = HealthTracker::new(
        default_group.failover.failure_threshold,
        default_group.failover.recovery_timeout,
    );

    // 创建路由引擎
    let routing_engine = RoutingEngine::new(health_tracker);

    // 获取第一个 provider 的 ssl_verify 作为 HTTP 客户端配置
    let ssl_verify = default_group
        .providers
        .first()
        .map(|p| p.ssl_verify)
        .unwrap_or(true);

    // 创建 HTTP 客户端
    let http_client = reqwest::Client::builder()
        .danger_accept_invalid_certs(!ssl_verify)
        .build()
        .map_err(error::RouterError::Network)?;

    // 创建 Providers
    let mut providers: Vec<Arc<dyn Provider>> = Vec::new();
    for provider_config in &default_group.providers {
        let provider = OpenAIProvider::new(provider_config.clone())
            .map_err(|e| error::RouterError::Config(format!("Failed to create provider '{}': {}", provider_config.name, e)))?;
        providers.push(Arc::new(provider) as Arc<dyn Provider>);
    }

    tracing::info!("Initialized {} providers", providers.len());

    // 创建应用状态
    let state = Arc::new(AppState {
        config: config.clone(),
        routing_engine,
        http_client,
        providers,
    });

    // 创建 Axum 路由
    let app = create_router(state);

    // 启动服务器
    let addr = format!("{}:{}", config.server.host, config.server.port);
    tracing::info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

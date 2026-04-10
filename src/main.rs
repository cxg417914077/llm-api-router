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

    // 验证至少有一个组
    if config.groups.is_empty() {
        return Err(crate::error::RouterError::Config("No provider groups configured".to_string()).into());
    }

    // 获取第一个组的故障转移配置用于健康追踪器
    let first_group = config
        .groups
        .values()
        .next()
        .ok_or_else(|| crate::error::RouterError::Config("No provider groups configured".to_string()))?;

    // 创建健康追踪器
    let health_tracker = HealthTracker::new(
        first_group.failover.failure_threshold,
        first_group.failover.recovery_timeout,
    );

    // 创建路由引擎
    let routing_engine = RoutingEngine::new(health_tracker);

    // 创建 Providers（所有组的所有 providers）
    let mut providers: Vec<Arc<dyn Provider>> = Vec::new();
    for (group_name, group_config) in &config.groups {
        for provider_config in &group_config.providers {
            let provider = OpenAIProvider::new(provider_config.clone())
                .map_err(|e| error::RouterError::Config(format!("Failed to create provider '{}.{}': {}", group_name, provider_config.name, e)))?;
            providers.push(Arc::new(provider) as Arc<dyn Provider>);
        }
    }

    tracing::info!("Initialized {} providers across {} groups", providers.len(), config.groups.len());

    // 创建应用状态
    let state = Arc::new(AppState {
        config: config.clone(),
        routing_engine,
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

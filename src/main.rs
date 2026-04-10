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

    // 创建健康追踪器
    let health_tracker = HealthTracker::new(
        config.failover.failure_threshold,
        config.failover.recovery_timeout,
    );

    // 创建路由引擎
    let routing_engine = RoutingEngine::new(health_tracker);

    // 创建 HTTP 客户端
    let http_client = reqwest::Client::builder()
        .danger_accept_invalid_certs(!config.ssl_verify)
        .build()
        .map_err(|e| error::RouterError::Network(e))?;

    // 创建 Providers
    let providers: Vec<Arc<dyn Provider>> = config
        .providers
        .iter()
        .map(|c| {
            Arc::new(OpenAIProvider::new(c.clone())) as Arc<dyn Provider>
        })
        .collect();

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

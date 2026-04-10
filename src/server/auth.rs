use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;

use crate::server::AppState;

/// API Key 认证中间件
/// 验证请求头中的 Authorization: Bearer <api_key>
pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // 从请求头中获取 Authorization
    let auth_header = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());

    let provided_key = match auth_header {
        Some(header) => {
            // 支持两种格式：
            // 1. "Bearer sk-xxx"
            // 2. "sk-xxx" (直接 API Key)
            if let Some(key) = header.strip_prefix("Bearer ") {
                key
            } else {
                header
            }
        }
        None => {
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    // 验证 API Key
    if provided_key != state.config.router.api_key {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // 认证通过，继续处理请求
    Ok(next.run(request).await)
}

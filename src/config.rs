use serde::Deserialize;
use std::env;

use crate::error::Result;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub server: ServerConfig,
    #[serde(default = "default_ssl_verify")]
    pub ssl_verify: bool,
    #[serde(default)]
    pub failover: FailoverConfig,
    pub providers: Vec<ProviderConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct FailoverConfig {
    #[serde(default = "default_failure_threshold")]
    pub failure_threshold: u32,
    #[serde(default = "default_recovery_timeout")]
    pub recovery_timeout: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ProviderConfig {
    pub name: String,
    pub endpoint: String,
    pub api_key: String,
    pub priority: u32,
    #[serde(default)]
    pub models: Vec<String>,
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    8080
}

fn default_ssl_verify() -> bool {
    true
}

fn default_failure_threshold() -> u32 {
    3
}

fn default_recovery_timeout() -> u64 {
    60
}

impl Config {
    pub fn load() -> Result<Self> {
        dotenvy::dotenv().ok();

        let config_path = std::env::var("CONFIG_PATH")
            .unwrap_or_else(|_| "config.yaml".to_string());

        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| crate::error::RouterError::Config(format!("Failed to read config file: {}", e)))?;
        let mut config: Config = serde_yaml::from_str(&content)
            .map_err(|e| crate::error::RouterError::Config(format!("Failed to parse config: {}", e)))?;

        // 展开环境变量
        for provider in &mut config.providers {
            provider.api_key = expand_env(&provider.api_key);
        }

        Ok(config)
    }
}

fn expand_env(s: &str) -> String {
    if let Some(var_name) = s.strip_prefix("${").and_then(|s| s.strip_suffix("}")) {
        env::var(var_name).unwrap_or_else(|_| s.to_string())
    } else {
        s.to_string()
    }
}

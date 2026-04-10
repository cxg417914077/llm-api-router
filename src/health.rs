use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct ProviderHealth {
    pub failure_count: u32,
    pub is_healthy: bool,
    pub last_failure_time: Option<Instant>,
}

impl ProviderHealth {
    fn new() -> Self {
        Self {
            failure_count: 0,
            is_healthy: true,
            last_failure_time: None,
        }
    }
}

/// ProviderKey 用于唯一标识 (group, provider) 对
/// 同一个 Provider 名称可能在不同组中重复出现
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ProviderKey {
    pub group: String,
    pub provider: String,
}

impl ProviderKey {
    pub fn new(group: &str, provider: &str) -> Self {
        Self {
            group: group.to_string(),
            provider: provider.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct HealthTracker {
    providers: Arc<Mutex<HashMap<ProviderKey, ProviderHealth>>>,
    failure_threshold: u32,
    recovery_timeout: u64,
}

impl HealthTracker {
    pub fn new(failure_threshold: u32, recovery_timeout: u64) -> Self {
        Self {
            providers: Arc::new(Mutex::new(HashMap::new())),
            failure_threshold,
            recovery_timeout,
        }
    }

    pub fn record_success(&self, group: &str, provider_name: &str) {
        let key = ProviderKey::new(group, provider_name);
        let mut providers = self.providers.lock().unwrap();
        let health = providers
            .entry(key)
            .or_insert_with(ProviderHealth::new);
        health.failure_count = 0;
        health.is_healthy = true;
    }

    pub fn record_failure(&self, group: &str, provider_name: &str) {
        let key = ProviderKey::new(group, provider_name);
        let mut providers = self.providers.lock().unwrap();
        let health = providers
            .entry(key)
            .or_insert_with(ProviderHealth::new);
        health.failure_count += 1;
        health.last_failure_time = Some(Instant::now());

        if health.failure_count >= self.failure_threshold {
            health.is_healthy = false;
        }
    }

    pub fn is_healthy(&self, group: &str, provider_name: &str) -> bool {
        let key = ProviderKey::new(group, provider_name);
        let mut providers = self.providers.lock().unwrap();
        let health = providers
            .entry(key)
            .or_insert_with(ProviderHealth::new);

        // 检查是否应该恢复健康
        if !health.is_healthy {
            if let Some(last_failure) = health.last_failure_time {
                if last_failure.elapsed() >= Duration::from_secs(self.recovery_timeout) {
                    health.is_healthy = true;
                    health.failure_count = 0;
                }
            }
        }

        health.is_healthy
    }

    pub fn get_healthy_providers(&self, group: &str, all_providers: &[String]) -> Vec<String> {
        all_providers
            .iter()
            .filter(|name| self.is_healthy(group, name))
            .cloned()
            .collect()
    }
}

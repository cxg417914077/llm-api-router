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

#[derive(Debug, Clone)]
pub struct HealthTracker {
    providers: Arc<Mutex<HashMap<String, ProviderHealth>>>,
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

    pub fn record_success(&self, provider_name: &str) {
        let mut providers = self.providers.lock().unwrap();
        let health = providers
            .entry(provider_name.to_string())
            .or_insert_with(ProviderHealth::new);
        health.failure_count = 0;
        health.is_healthy = true;
    }

    pub fn record_failure(&self, provider_name: &str) {
        let mut providers = self.providers.lock().unwrap();
        let health = providers
            .entry(provider_name.to_string())
            .or_insert_with(ProviderHealth::new);
        health.failure_count += 1;
        health.last_failure_time = Some(Instant::now());

        if health.failure_count >= self.failure_threshold {
            health.is_healthy = false;
        }
    }

    pub fn is_healthy(&self, provider_name: &str) -> bool {
        let mut providers = self.providers.lock().unwrap();
        let health = providers
            .entry(provider_name.to_string())
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

    pub fn get_healthy_providers(&self, all_providers: &[String]) -> Vec<String> {
        all_providers
            .iter()
            .filter(|name| self.is_healthy(name))
            .cloned()
            .collect()
    }
}

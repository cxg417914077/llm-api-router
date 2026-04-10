use crate::config::ProviderConfig;
use crate::health::HealthTracker;

pub struct RoutingEngine {
    health_tracker: HealthTracker,
}

impl RoutingEngine {
    pub fn new(health_tracker: HealthTracker) -> Self {
        Self { health_tracker }
    }

    /// 根据优先级选择下一个健康的 provider（组内路由）
    pub fn select_provider<'a>(
        &self,
        group: &str,
        providers: &'a [ProviderConfig],
        last_attempted: Option<&str>,
    ) -> Option<&'a ProviderConfig> {
        // 按优先级排序
        let mut sorted: Vec<_> = providers.iter().collect();
        sorted.sort_by_key(|p| p.priority);

        // 获取健康的 providers（按组过滤）
        let healthy_names: Vec<_> = self
            .health_tracker
            .get_healthy_providers(group, &sorted.iter().map(|p| p.name.clone()).collect::<Vec<_>>());

        // 找到第一个健康的、且不是上次尝试过的 provider
        for provider in &sorted {
            if healthy_names.contains(&provider.name) {
                if let Some(last) = last_attempted {
                    if provider.name == last {
                        continue;
                    }
                }
                return Some(*provider);
            }
        }

        // 如果所有健康的都试过了，返回第一个（即使不健康也要尝试）
        sorted.first().copied()
    }

    pub fn health_tracker(&self) -> &HealthTracker {
        &self.health_tracker
    }
}

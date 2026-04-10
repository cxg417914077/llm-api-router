use crate::health::HealthTracker;

pub struct RoutingEngine {
    health_tracker: HealthTracker,
}

impl RoutingEngine {
    pub fn new(health_tracker: HealthTracker) -> Self {
        Self { health_tracker }
    }

    pub fn health_tracker(&self) -> &HealthTracker {
        &self.health_tracker
    }
}

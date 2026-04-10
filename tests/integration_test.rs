#[tokio::test]
async fn test_health_tracker_per_model() {
    // 测试健康追踪器按 (group, provider, model) 三元组追踪的功能
    use llm_api_router::health::HealthTracker;

    let tracker = HealthTracker::new(3, 60);

    // 初始状态应该是健康的
    assert!(tracker.is_healthy("production", "primary", "gpt-4o"));
    assert!(tracker.is_healthy("production", "primary", "gpt-4o-mini"));
    assert!(tracker.is_healthy("staging", "primary", "gpt-4o"));

    // 在同一 provider 的不同 model 上记录失败，应该独立追踪
    tracker.record_failure("production", "primary", "gpt-4o");
    tracker.record_failure("production", "primary", "gpt-4o");

    // production/primary/gpt-4o 仍然健康（2 次失败 < 3 次阈值）
    assert!(tracker.is_healthy("production", "primary", "gpt-4o"));
    // production/primary/gpt-4o-mini 应该仍然健康（没有失败）
    assert!(tracker.is_healthy("production", "primary", "gpt-4o-mini"));

    // 第 3 次失败，production/primary/gpt-4o 不健康
    tracker.record_failure("production", "primary", "gpt-4o");
    assert!(!tracker.is_healthy("production", "primary", "gpt-4o"));

    // production/primary/gpt-4o-mini 仍然健康
    assert!(tracker.is_healthy("production", "primary", "gpt-4o-mini"));

    // 记录成功，恢复健康
    tracker.record_success("production", "primary", "gpt-4o");
    assert!(tracker.is_healthy("production", "primary", "gpt-4o"));
}

#[tokio::test]
async fn test_provider_key_equality() {
    use llm_api_router::health::ProviderKey;

    // 相同组、provider、model 的 ProviderKey 应该相等
    let key1 = ProviderKey::new("production", "primary", "gpt-4o");
    let key2 = ProviderKey::new("production", "primary", "gpt-4o");
    assert_eq!(key1, key2);

    // 不同组的 ProviderKey 应该不相等
    let key3 = ProviderKey::new("staging", "primary", "gpt-4o");
    assert_ne!(key1, key3);

    // 同组不同 Provider 应该不相等
    let key4 = ProviderKey::new("production", "secondary", "gpt-4o");
    assert_ne!(key1, key4);

    // 同 provider 不同 model 应该不相等
    let key5 = ProviderKey::new("production", "primary", "gpt-4o-mini");
    assert_ne!(key1, key5);
}

#[tokio::test]
async fn test_config_groups_structure() {
    // 测试配置结构可以正确序列化/反序列化（v2 groups 格式）
    use serde_yaml;

    let config_content = r#"
router:
  api_key: "sk-test-key"
server:
  host: "127.0.0.1"
  port: 9090
groups:
  production:
    failover:
      failure_threshold: 5
      recovery_timeout: 30
    providers:
      - name: "primary"
        endpoint: "https://api.openai.com/v1"
        api_key: "${OPENAI_API_KEY}"
        priority: 1
        ssl_verify: true
      - name: "fallback"
        endpoint: "http://localhost:8000/v1"
        api_key: "not-needed"
        priority: 2
        ssl_verify: false
  staging:
    failover:
      failure_threshold: 3
      recovery_timeout: 60
    providers:
      - name: "staging-primary"
        endpoint: "https://api.openai.com/v1"
        api_key: "${OPENAI_API_KEY}"
        priority: 1
        ssl_verify: true
"#;

    let config: serde_yaml::Value = serde_yaml::from_str(config_content).unwrap();

    // 验证 router 配置
    assert_eq!(config["router"]["api_key"], "sk-test-key");

    // 验证 server 配置
    assert_eq!(config["server"]["host"], "127.0.0.1");
    assert_eq!(config["server"]["port"], 9090);

    // 验证 groups 结构
    assert!(config["groups"]["production"].is_mapping());
    assert!(config["groups"]["staging"].is_mapping());

    // 验证 production 组配置
    assert_eq!(
        config["groups"]["production"]["failover"]["failure_threshold"],
        5
    );
    assert_eq!(
        config["groups"]["production"]["failover"]["recovery_timeout"],
        30
    );
    assert_eq!(
        config["groups"]["production"]["providers"]
            .as_sequence()
            .unwrap()
            .len(),
        2
    );

    // 验证 provider 配置
    let primary = &config["groups"]["production"]["providers"][0];
    assert_eq!(primary["name"], "primary");
    assert_eq!(primary["ssl_verify"], true);

    let fallback = &config["groups"]["production"]["providers"][1];
    assert_eq!(fallback["name"], "fallback");
    assert_eq!(fallback["ssl_verify"], false);
}

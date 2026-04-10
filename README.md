# LLM API Router

高可用的 LLM API 路由器，支持优先级链式故障转移和多组路由。

## 特性

- ✅ **多组路由** - 通过 `model` 字段指定组名，灵活路由
- ✅ **优先级故障转移** - 主服务失败时自动切换到备用服务
- ✅ **被动健康检查** - 自动跳过频繁失败的节点
- ✅ **每 Provider SSL 配置** - 支持独立 SSL 验证配置
- ✅ **OpenAI 兼容** - 使用标准 OpenAI SDK 即可调用
- ✅ **API Key 认证** - 保护 Router 服务
- ✅ **配置驱动** - YAML 配置文件，支持环境变量

## 快速开始

### 1. 配置

复制示例配置并修改：

```bash
cp config.yaml.example config.yaml
cp .env.example .env
```

编辑 `config.yaml` 设置你的 Provider 组：

```yaml
# Router 自身配置
router:
  api_key: "sk-your-router-key"

# 服务器配置
server:
  host: "0.0.0.0"
  port: 8080

# Provider 组定义
groups:
  production:
    failover:
      failure_threshold: 3
      recovery_timeout: 60
    providers:
      - name: "primary"
        endpoint: "https://api.openai.com/v1"
        api_key: "${OPENAI_API_KEY}"
        priority: 1
        ssl_verify: true

      - name: "local-ollama"
        endpoint: "http://localhost:8000/v1"
        api_key: "not-needed"
        priority: 2
        ssl_verify: false

  staging:
    failover:
      failure_threshold: 5
      recovery_timeout: 30
    providers:
      - name: "staging-openai"
        endpoint: "https://api.openai.com/v1"
        api_key: "${OPENAI_API_KEY}"
        priority: 1
        ssl_verify: true
```

### 2. 运行

```bash
cargo run --release
```

### 3. 使用

使用 OpenAI SDK 调用：

```python
from openai import OpenAI

client = OpenAI(
    base_url="http://localhost:8080/v1",
    api_key="sk-your-router-key"  # Router 的 API Key
)

# 使用 production 组
response = client.chat.completions.create(
    model="production",  # model 字段指定组名
    messages=[{"role": "user", "content": "Hello"}]
)

print(response.choices[0].message.content)
```

或使用 curl：

```bash
# 调用 production 组
curl http://localhost:8080/v1/chat/completions \
  -H "Authorization: Bearer sk-your-router-key" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "production",
    "messages": [{"role": "user", "content": "Hello"}]
  }'

# 调用 staging 组
curl http://localhost:8080/v1/chat/completions \
  -H "Authorization: Bearer sk-your-router-key" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "staging",
    "messages": [{"role": "user", "content": "Hello"}]
  }'
```

## 配置选项

### Router 配置

| 字段 | 类型 | 说明 |
|------|------|------|
| `router.api_key` | string | Router 服务的 API Key，用于认证 |

### 服务器配置

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `server.host` | string | "0.0.0.0" | 监听地址 |
| `server.port` | u16 | 8080 | 监听端口 |

### 故障转移配置（每组独立）

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `failover.failure_threshold` | u32 | 3 | 失败多少次后标记为不健康 |
| `failover.recovery_timeout` | u64 | 60 | 多少秒后允许重试不健康的节点 |

### Provider 配置

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | string | Provider 唯一标识 |
| `endpoint` | string | 下游 API 端点 |
| `api_key` | string | API Key（支持 `${ENV_VAR}` 语法） |
| `priority` | u32 | 优先级（数字越小优先级越高） |
| `ssl_verify` | bool | 是否验证下游 SSL 证书（默认 true） |

## 故障转移逻辑

1. 请求的 `model` 字段指定组名
2. 在该组内按 priority 升序尝试 providers
3. 失败（5xx/超时/429/业务错误）时记录失败计数
4. 达到 `failure_threshold` 后标记为不健康，跳过
5. `recovery_timeout` 后允许重试
6. 所有 provider 失败时返回错误

## 健康追踪

- 健康状态按 `(group, provider)` 键独立追踪
- 同一 Provider 名称在不同组中有独立的健康状态
- 失败计数在成功请求后清零
- 不健康节点在 `recovery_timeout` 秒后自动尝试恢复

## 开发

```bash
# 运行
cargo run

# 测试
cargo test

# 检查
cargo clippy

# 格式化
cargo fmt
```

## License

MIT

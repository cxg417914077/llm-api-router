# LLM API Router

高可用的 LLM API 路由器，支持优先级链式故障转移。

## 特性

- ✅ **优先级故障转移** - 主服务失败时自动切换到备用服务
- ✅ **被动健康检查** - 自动跳过频繁失败的节点
- ✅ **OpenAI 兼容** - 使用标准 OpenAI SDK 即可调用
- ✅ **SSL 验证开关** - 支持本地自签名证书
- ✅ **配置驱动** - YAML 配置文件，支持环境变量

## 快速开始

### 1. 配置

复制示例配置并修改：

```bash
cp config.yaml.example config.yaml
cp .env.example .env
```

编辑 `config.yaml` 设置你的 providers：

```yaml
server:
  host: "0.0.0.0"
  port: 8080

ssl_verify: true

failover:
  failure_threshold: 3
  recovery_timeout: 60

providers:
  - name: "primary"
    endpoint: "https://api.openai.com/v1"
    api_key: "${OPENAI_API_KEY}"
    priority: 1
    models: ["gpt-4", "gpt-3.5-turbo"]
    
  - name: "fallback"
    endpoint: "http://localhost:8000/v1"
    api_key: "not-needed"
    priority: 2
    models: ["*"]
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
    api_key="not-needed"  # Router 的 API Key，可自定义
)

response = client.chat.completions.create(
    model="any-model",
    messages=[{"role": "user", "content": "Hello"}]
)

print(response.choices[0].message.content)
```

或使用 curl：

```bash
curl http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4",
    "messages": [{"role": "user", "content": "Hello"}]
  }'
```

## 配置选项

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `server.host` | string | "0.0.0.0" | 监听地址 |
| `server.port` | u16 | 8080 | 监听端口 |
| `ssl_verify` | bool | true | 是否验证下游 SSL 证书 |
| `failover.failure_threshold` | u32 | 3 | 失败多少次后标记为不健康 |
| `failover.recovery_timeout` | u64 | 60 | 多少秒后允许重试不健康的节点 |

### Provider 配置

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | string | Provider 唯一标识 |
| `endpoint` | string | 下游 API 端点 |
| `api_key` | string | API Key（支持 ${ENV_VAR} 语法） |
| `priority` | u32 | 优先级（数字越小优先级越高） |
| `models` | string[] | 支持的模型列表（["*"] 表示全部） |

## 故障转移逻辑

1. 请求按 priority 升序尝试 providers
2. 失败（5xx/超时/429/业务错误）时记录失败计数
3. 达到 `failure_threshold` 后标记为不健康，跳过
4. `recovery_timeout` 后允许重试
5. 所有 provider 失败时返回错误

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

# LLM API Router

<div align="center">

**高可用的 LLM API 路由器 · 支持优先级链式故障转移 · 细粒度健康检查**

[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

</div>

---

LLM API Router 是一个用 Rust 编写的高性能 API 网关，专为 LLM 服务设计。它支持多组路由、优先级故障转移和细粒度的健康检查，确保你的服务始终可用。

## ✨ 特性

- 🎯 **多组路由** - 通过 `model` 字段指定组名，灵活路由到不同服务组
- ⛓️ **链式故障转移** - 主服务失败时自动切换到备用服务，支持两层优先级（Provider → Model）
- ❤️ **细粒度健康检查** - 精确到 `(组，Provider, Model)` 三元组，自动跳过失败的模型
- 🔒 **每 Provider SSL 配置** - 支持独立 SSL 验证配置，适配不同下游服务
- 🔑 **API Key 认证** - 保护 Router 服务，防止未授权访问
- 📝 **配置驱动** - YAML 配置文件，支持环境变量替换
- 🚀 **OpenAI 兼容** - 使用标准 OpenAI SDK 即可调用，零学习成本
- 🦀 **Rust 编写** - 高性能、低延迟、内存安全

## 🚀 快速开始

### 安装

```bash
# 克隆仓库
git clone https://github.com/YOUR_USERNAME/llm-api-router.git
cd llm-api-router

# 复制配置文件
cp config.yaml.example config.yaml
cp .env.example .env

# 编辑配置
vim config.yaml
```

### 配置示例

```yaml
# Router 自身配置
router:
  api_key: "sk-your-router-key"  # Router 的 API Key

# 服务器配置
server:
  host: "0.0.0.0"
  port: 8080

# Provider 组定义
groups:
  # 生产环境组
  production:
    failover:
      failure_threshold: 3      # 失败 3 次后标记为不健康
      recovery_timeout: 60      # 60 秒后尝试恢复
    providers:
      - name: "openai-primary"
        endpoint: "https://api.openai.com/v1"
        api_key: "${OPENAI_API_KEY}"
        priority: 1              # 最高优先级
        ssl_verify: true
        models:                  # 按优先级排序的模型列表
          - "gpt-4o"
          - "gpt-4o-mini"
          - "gpt-3.5-turbo"

      - name: "azure-backup"
        endpoint: "https://your-resource.openai.azure.com/openai/deployments/gpt-4o"
        api_key: "${AZURE_API_KEY}"
        priority: 2              # 备用优先级
        ssl_verify: true
        models:
          - "gpt-4o"

  # 本地测试组
  local:
    failover:
      failure_threshold: 2
      recovery_timeout: 30
    providers:
      - name: "ollama"
        endpoint: "http://localhost:11434/v1"
        api_key: "not-needed"
        priority: 1
        ssl_verify: false
        models:
          - "llama3"
          - "qwen2.5"
```

### 环境变量

创建 `.env` 文件：

```bash
OPENAI_API_KEY=sk-your-openai-key
AZURE_API_KEY=your-azure-key
```

### 运行

```bash
cargo run --release
```

### 使用

#### Python (OpenAI SDK)

```python
from openai import OpenAI

client = OpenAI(
    base_url="http://localhost:8080/v1",
    api_key="sk-your-router-key"  # Router 的 API Key
)

# 调用 production 组，自动在组内故障转移
response = client.chat.completions.create(
    model="production",  # model 字段指定组名
    messages=[{"role": "user", "content": "Hello"}]
)

print(response.choices[0].message.content)
```

#### curl

```bash
# 调用 production 组
curl http://localhost:8080/v1/chat/completions \
  -H "Authorization: Bearer sk-your-router-key" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "production",
    "messages": [{"role": "user", "content": "Hello"}]
  }'
```

## 📖 配置说明

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
| `models` | string[] | 支持的模型列表，按优先级排序 |

## 🔄 故障转移逻辑

### 两层优先级设计

```
请求：model="production"
        │
        ▼
┌───────────────────────────────────┐
│  Group: production                │
│  ┌─────────────────────────────┐  │
│  │ Provider: openai-primary    │  │
│  │ priority: 1 (最高)          │  │
│  │ models: [gpt-4o, gpt-4o-   │  │
│  │           mini, gpt-3.5]   │  │
│  └─────────────────────────────┘  │
│  ┌─────────────────────────────┐  │
│  │ Provider: azure-backup      │  │
│  │ priority: 2 (备用)          │  │
│  │ models: [gpt-4o]            │  │
│  └─────────────────────────────┘  │
└───────────────────────────────────┘
```

### 详细流程

1. 客户端请求的 `model` 字段指定**组名**（如 `model="production"`）
2. Router 在该组内按 `priority` 升序选择 Provider
3. 对于每个 Provider，按 `models` 列表顺序选择模型：
   - 检查该 `(组，Provider, Model)` 三元组的健康状态
   - 如果健康，发送请求
   - 如果失败，记录失败并尝试下一个模型
4. 如果 Provider 的所有模型都失败，切换到下一个 Provider
5. 所有 Provider 都失败时返回错误

### 健康追踪

- **细粒度追踪**：健康状态按 `(组，Provider, Model)` 三元组独立追踪
- **自动恢复**：不健康的模型在 `recovery_timeout` 秒后自动尝试恢复
- **成功清零**：请求成功后，失败计数清零

## 📊 日志示例

```bash
# 第一次请求 - gpt-4o 失败，自动切换到 gpt-4o-mini
2026-04-10T14:50:44.600669Z  INFO 🔀 收到请求：组名='production', 原始 model='production'
2026-04-10T14:50:44.600712Z  INFO 🎯 Provider 'openai-primary' 选择 model='gpt-4o' (models[0], 健康检查通过)
2026-04-10T14:50:46.459526Z  WARN ❌ Provider openai-primary (model: gpt-4o) 失败：HttpError { status: 500, ... }
2026-04-10T14:50:46.459575Z  INFO 🎯 Provider 'openai-primary' 选择 model='gpt-4o-mini' (models[1], 健康检查通过)
2026-04-10T14:50:54.531695Z  INFO ✅ 请求成功：provider='openai-primary', model='gpt-4o-mini'

# 第二次请求 - 自动跳过 gpt-4o，直接尝试 gpt-4o-mini
2026-04-10T14:51:08.670353Z  INFO ⏭️  Provider 'openai-primary' 跳过不健康的 model='gpt-4o' (models[0])
2026-04-10T14:51:08.670357Z  INFO 🎯 Provider 'openai-primary' 选择 model='gpt-4o-mini' (models[1], 健康检查通过)
2026-04-10T14:51:15.136005Z  INFO ✅ 请求成功：provider='openai-primary', model='gpt-4o-mini'
```

## 🛠️ 开发

```bash
# 运行
cargo run

# 测试
cargo test

# 代码检查
cargo clippy

# 代码格式化
cargo fmt
```

## 📦 项目结构

```
llm-api-router/
├── src/
│   ├── main.rs          # 程序入口
│   ├── lib.rs           # 库导出
│   ├── config.rs        # 配置加载与解析
│   ├── error.rs         # 错误类型定义
│   ├── health.rs        # 健康追踪器
│   ├── routing.rs       # 路由引擎
│   ├── provider/
│   │   ├── mod.rs       # Provider  trait 定义
│   │   └── openai.rs    # OpenAI Provider 实现
│   └── server/
│       ├── mod.rs       # 服务器模块
│       ├── handlers.rs  # 请求处理器
│       └── auth.rs      # API Key 认证中间件
├── tests/
│   └── integration_test.rs  # 集成测试
├── config.yaml.example  # 配置示例
├── .env.example         # 环境变量示例
└── README.md            # 本文档
```

## 🤝 贡献

欢迎提交 Issue 和 Pull Request！

## 📄 License

MIT License

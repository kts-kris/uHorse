# uHorse 测试指南

本文档只描述 **当前仓库真实可对齐的测试路径**，重点是：

- 编译检查
- 包级测试
- 本地 Hub-Node roundtrip
- DingTalk / LLM 启动验证

不再把旧单体 `uhorse`、旧 `/health/live`、旧 Kubernetes 健康检查当作默认测试入口。

## 目录

- [快速命令](#快速命令)
- [编译检查](#编译检查)
- [常用测试命令](#常用测试命令)
- [真实本地 roundtrip 测试](#真实本地-roundtrip-测试)
- [手动本机验证](#手动本机验证)
- [DingTalk 与 LLM 验证](#dingtalk-与-llm-验证)
- [当前边界](#当前边界)
- [下一步](#下一步)

---

## 快速命令

```bash
cargo build --release -p uhorse-hub -p uhorse-node
cargo test -p uhorse-hub
cargo test -p uhorse-node
cargo test -p uhorse-hub test_local_hub_node_roundtrip_file_exists -- --nocapture
```

---

## 编译检查

### 编译 Hub 和 Node

```bash
cargo build -p uhorse-hub
cargo build -p uhorse-node
```

### Release 编译

```bash
cargo build --release -p uhorse-hub -p uhorse-node
```

### 常用检查

```bash
cargo check
cargo fmt --check
cargo clippy --all-targets --all-features
```

---

## 常用测试命令

### 运行 Hub 包测试

```bash
cargo test -p uhorse-hub
```

### 运行 Node 包测试

```bash
cargo test -p uhorse-node
```

### 带输出运行

```bash
cargo test -p uhorse-hub -- --nocapture
cargo test -p uhorse-node -- --nocapture
```

### 只跑某条测试

```bash
cargo test -p uhorse-hub test_hub_creation -- --nocapture
cargo test -p uhorse-hub test_local_hub_node_roundtrip_file_exists -- --nocapture
```

---

## 真实本地 roundtrip 测试

当前最关键、最能说明本地闭环打通的测试是：

```bash
cargo test -p uhorse-hub test_local_hub_node_roundtrip_file_exists -- --nocapture
```

它会真实验证：

1. Hub 启动
2. WebSocket 服务启动
3. Node 启动并连接 Hub
4. Hub 下发文件命令
5. Node 执行命令
6. Node 回传 `TaskResult`
7. Hub 收到结果并完成任务

如果这条测试通过，说明 **Hub → Node → Hub** 的最小本地闭环已打通。

---

## 手动本机验证

### 1. 生成配置

```bash
./target/release/uhorse-hub init --output hub.toml
./target/release/uhorse-node init --output node.toml
```

### 2. 启动 Hub

```bash
./target/release/uhorse-hub --config hub.toml --log-level info
```

### 3. 启动 Node

```bash
./target/release/uhorse-node --config node.toml --log-level info
```

### 4. 检查健康状态与在线节点

```bash
curl http://127.0.0.1:8765/api/health
curl http://127.0.0.1:8765/api/nodes
```

### 5. 检查 Node 工作空间访问

```bash
./target/release/uhorse-node check --workspace .
```

---

## DingTalk 与 LLM 验证

### DingTalk Stream 初始化验证

让 `hub.toml` 使用统一配置，并包含：

```toml
[channels]
enabled = ["dingtalk"]

[channels.dingtalk]
app_key = "your_app_key"
app_secret = "your_app_secret"
agent_id = 123456789
```

启动 Hub 后，重点看日志里是否出现 DingTalk 初始化成功，以及任务完成后的 DingTalk 回传日志。

### LLM 初始化验证

统一配置示例：

```toml
[llm]
enabled = true
provider = "custom-provider"
api_key = "your_api_key"
base_url = "https://api.example.com/v1"
model = "your-model"
temperature = 0.7
max_tokens = 2000
system_prompt = "You are a helpful AI assistant for uHorse."
```

当前代码支持：

- `openai`
- `azure_openai`
- `anthropic`
- `gemini`
- 任意自定义 provider 字符串

其中自定义 provider 会按 **OpenAI 兼容端点** 处理，也就是请求：

```text
{base_url}/chat/completions
```

并使用 Bearer Token。

### 当前手工验证重点

- Hub 是否正确加载统一配置
- DingTalk channel 是否成功启动
- LLM client 是否成功初始化
- `/api/nodes` 是否能看到在线 Node
- 本地 roundtrip 是否成功
- 自然语言请求是否被规划为受本地校验约束的 `FileCommand` / `ShellCommand`
- 真实 DingTalk 会话里是否能看到错误即时回显与成功结果原路回传
- 结果总结失败时是否能回退到结构化文本

---

## 当前边界

以下内容不要误认为已经是默认已验证路径：

- 旧单体 `uhorse` 的 `/health/live` / `/health/ready`
- 旧 `OPENCLAW_*` 环境变量链路
- `deployments/k8s/base/*` 直接用于 v4.0 Hub-Node 生产部署
- 未提供真实企业凭据时，无法在你自己的环境复现 DingTalk 最后一跳联调

当前已经明确验证的是：

- 本机启动 Hub
- 本机启动 Node
- Node 连接 Hub
- Hub → Node → Hub 的真实 roundtrip 测试
- 真实企业租户下的 DingTalk 回传闭环：不安全请求即时报错，合法请求结果可原路回传

因此，如果你只是验证本地代码链路，可先用本地 roundtrip 测试；如果你要复现 DingTalk 最后一跳，仍需要准备你自己的真实配置。

---

## 下一步

- [README.md](README.md)：项目总览
- [CONFIG.md](CONFIG.md)：配置手册
- [CHANNELS.md](CHANNELS.md)：DingTalk Stream 说明
- [deployments/DEPLOYMENT_V4.md](deployments/DEPLOYMENT_V4.md)：v4.0 部署路径

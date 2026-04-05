# uHorse 测试指南

本文档只描述 **当前仓库真实可对齐的测试路径**，重点覆盖：

- 编译检查
- 包级测试
- 工作区测试
- 真实本地 Hub-Node roundtrip
- JWT / 审批 / 重连手工回归

不再把旧单体 `uhorse`、旧 `/health/live`、旧 `/health/ready`、旧 `/api/v1/auth/*` 当作默认测试入口。

## 目录

- [快速命令](#快速命令)
- [编译检查](#编译检查)
- [包级测试](#包级测试)
- [关键回归测试](#关键回归测试)
- [手工 Hub-Node 回归顺序](#手工-hub-node-回归顺序)
- [DingTalk 与 LLM 验证](#dingtalk-与-llm-验证)
- [当前边界](#当前边界)

---

## 快速命令

```bash
cargo build --release -p uhorse-hub -p uhorse-node
cargo test -p uhorse-node-runtime
cargo test -p uhorse-hub
cargo test --workspace
cargo test -p uhorse-hub test_local_hub_node_roundtrip_file_exists -- --nocapture
cargo test -p uhorse-hub test_local_hub_rejects_node_with_mismatched_auth_token -- --nocapture
make skill-install-smoke
make agent-loop-smoke
make approval-loop-smoke
make observability-smoke
make audit-smoke
make test-quick
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

### 常用静态检查

```bash
cargo check
cargo fmt --check
cargo clippy --all-targets --all-features
```

---

## 包级测试

### Node 运行时测试

```bash
cargo test -p uhorse-node-runtime
```

### Hub 测试

```bash
cargo test -p uhorse-hub
```

### 工作区测试

```bash
cargo test --workspace
```

### 带输出运行

```bash
cargo test -p uhorse-node-runtime -- --nocapture
cargo test -p uhorse-hub -- --nocapture
```

---

## 关键回归测试

### 1. 本地真实 roundtrip

```bash
cargo test -p uhorse-hub test_local_hub_node_roundtrip_file_exists -- --nocapture
```

这条测试会真实验证：

1. Hub 启动
2. WebSocket 服务启动
3. Node 启动并连接 Hub
4. JWT 鉴权成功注册
5. Hub 下发 `FileCommand::Exists`
6. Node 执行后回传 `NodeToHub::TaskResult`
7. Hub 更新任务状态为 `Completed`

### 2. token 不匹配拒绝接入

```bash
cargo test -p uhorse-hub test_local_hub_rejects_node_with_mismatched_auth_token -- --nocapture
```

这条测试验证：

- Hub 已启用 `SecurityManager`
- token 内 `node_id` 与 Node 注册时提供的 `node_id` 不一致
- Hub 拒绝该 Node 上线

### 3. Agent Browser Skill 安装自动化回归

```bash
make skill-install-smoke
# 或
cargo test -p uhorse-hub test_agent_browser_natural_language_install_flow_returns_chinese_hint -- --nocapture
```

这条回归验证：

- “帮我安装 Agent Browser 技能”自然语言安装入口
- SkillHub 搜索结果解析与安装请求组装
- 运行时安装与 registry 刷新
- 安装成功后的简体中文功能介绍与使用示例

### 4. 安全与审批相关测试

```bash
cargo test -p uhorse-hub security_test -- --nocapture
```

重点覆盖：

- 审批需求判定
- 审批请求创建 / 批准 / 拒绝
- Node 发起审批请求后，Hub 是否创建对应审批项
- 未启用 `SecurityManager` 时的错误路径

### 5. Agent Loop continuation smoke

```bash
make agent-loop-smoke
# 或
cargo test -p uhorse-hub test_reply_task_result_records_compaction_and_retries_once -- --nocapture
cargo test -p uhorse-hub test_project_transcript_messages_includes_intermediate_events -- --nocapture
```

这组回归验证：

- planner → dispatch → task result continuation 主链路
- continuation compaction 与 planner retry 记录
- transcript 是否包含中间 loop 事件

### 6. approval wait / resume smoke

```bash
make approval-loop-smoke
# 或
cargo test -p uhorse-hub test_approval_request_records_wait_metric_and_transcript -- --nocapture
cargo test -p uhorse-hub test_approve_approval_appends_transcript_event_for_bound_turn -- --nocapture
```

这组回归验证：

- Node 发起 `ApprovalRequest` 时是否记录 approval wait transcript / metrics
- approval approve 后是否为已绑定 turn 追加 transcript 事件
- approval wait / resume 是否进入统一 loop 观测模型

### 7. observability smoke

```bash
make observability-smoke
# 或
cargo test -p uhorse-observability test_metrics_exporter_returns_prometheus_payload -- --nocapture
cargo test -p uhorse-backup test_restore_lifecycle_records_audit_events -- --nocapture
```

这组回归验证：

- loop steps
- continuations
- approval waits / resumes
- planner retries
- backup restore start / complete / fail / rollback 审计事件

这些指标与审计事件是否能通过默认 smoke 入口稳定回归

### 8. audit smoke

```bash
make audit-smoke
# 或
cargo test -p uhorse-hub test_approval_decision_records_audit_events -- --nocapture
cargo test -p uhorse-node-runtime test_dangerous_git_command_records_audit_event -- --nocapture
cargo test -p uhorse-node-runtime test_checkpoint_and_restore_record_audit_events -- --nocapture
cargo test -p uhorse-backup test_restore_lifecycle_records_audit_events -- --nocapture
```

这组回归验证：

- approval approve / reject 审计事件
- dangerous git deny 审计事件
- workspace checkpoint / restore 审计事件
- backup restore lifecycle 审计事件

---

## 手工 Hub-Node 回归顺序

如果你要做一轮完整主线回归，建议按下面顺序：

### 1. 启动 Hub，并确保启用安全配置

统一配置最少需要：

```toml
[server]
host = "127.0.0.1"
port = 8765

[security]
jwt_secret = "replace-with-random-secret"
```

启动：

```bash
./target/release/uhorse-hub --config hub.toml --log-level info
```

### 2. 为稳定 `node_id` 签发 token

```bash
curl -X POST http://127.0.0.1:8765/api/node-auth/token \
  -H "Content-Type: application/json" \
  -d '{
    "node_id": "office-node-01",
    "credentials": "bootstrap-secret"
  }'
```

### 3. 启动 Node

```bash
./target/release/uhorse-node --config node.toml --log-level info
```

### 4. 验证健康状态与在线节点

```bash
curl http://127.0.0.1:8765/api/health
curl http://127.0.0.1:8765/metrics
curl http://127.0.0.1:8765/api/nodes
```

### 5. 提交文件任务并检查状态

```bash
curl -X POST http://127.0.0.1:8765/api/tasks \
  -H "Content-Type: application/json" \
  -d '{
    "command": {
      "type": "file",
      "action": "exists",
      "path": "/tmp/demo.txt"
    },
    "user_id": "api-user",
    "session_id": "api-session",
    "channel": "api"
  }'
```

然后查询：

```bash
curl http://127.0.0.1:8765/api/tasks/<task_id>
```

### 6. 推送权限规则并触发审批

先下发一个要求 shell 执行审批的规则，再提交 shell 任务。

### 7. 通过审批接口做决策

```bash
curl http://127.0.0.1:8765/api/approvals
curl -X POST http://127.0.0.1:8765/api/approvals/<request_id>/approve \
  -H "Content-Type: application/json" \
  -d '{"responder":"admin","reason":"approved"}'
```

或：

```bash
curl -X POST http://127.0.0.1:8765/api/approvals/<request_id>/reject \
  -H "Content-Type: application/json" \
  -d '{"responder":"admin","reason":"denied"}'
```

### 8. 确认最终任务状态

再次请求：

```bash
curl http://127.0.0.1:8765/api/tasks/<task_id>
```

确认状态进入 `Completed` 或 `Failed`。

### 9. 验证 Hub 重启后的 Node 自动重连

- 保持 Node 进程存活
- 重启 Hub
- 再次请求 `GET /api/nodes`
- 再提交一个任务，确认链路恢复正常

---

## DingTalk 与 LLM 验证

如果需要补做 DingTalk / LLM 启动验证，Hub 需使用统一配置。

### DingTalk Stream 最小配置

```toml
[channels]
enabled = ["dingtalk"]

[channels.dingtalk]
app_key = "your_app_key"
app_secret = "your_app_secret"
agent_id = 123456789

# 可选：仅在需要兼容 seed/fallback 时保留静态绑定
[[channels.dingtalk.notification_bindings]]
node_id = "your-stable-node-id"
user_id = "your-dingtalk-user-id"
```

> Node Desktop 通知镜像当前主路径是 pairing 运行时绑定；这里的静态 `notification_bindings` 仅用于兼容 seed/fallback。
>
> 验收时请以 Settings 页面**当前最新显示**的 6 位绑定码为准；如果点击过“重新生成绑定码”，旧码会立即失效。当前主线已修复 DingTalk Stream 入站绕过 pairing 处理的问题，绑定码消息会优先走 pairing 确认分支，而不是普通任务文本链路。

### LLM 最小配置

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

验证重点：

- Hub 是否正确加载统一配置
- DingTalk channel 是否成功初始化
- LLM client 是否成功初始化
- 自然语言请求是否被规划为受本地校验约束的 `FileCommand` / `ShellCommand`
- 结果总结失败时是否回退到结构化文本

---

## 当前边界

以下内容不要误认为已经是当前主线默认已验证路径：

- 旧 `/health/live` / `/health/ready`
- 旧 `/api/v1/auth/*`
- 旧 `/api/v1/messages`
- `GET /api/tasks` 返回真实任务列表

当前已经明确验证的是：

- `GET /api/health`
- `GET /metrics`
- `GET /api/nodes`
- `POST /api/node-auth/token`
- `POST /api/tasks`
- `GET /api/tasks/:task_id`
- `GET /api/approvals` + `/approve` + `/reject`
- Node 自动重连与重新注册

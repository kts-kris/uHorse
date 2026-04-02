# uHorse 通道指南

本文档只描述 **`v4.4.0` 当前仓库主线实际接入并在 Hub 运行时链路中使用的通道路径**。

当前最重要、也是主推荐路径的是：

- **DingTalk Stream 模式**

仓库中仍有 Telegram、Slack、Discord、WhatsApp、Feishu、WeCom 等通道模块，但当前 `uhorse-hub` 主运行时文档与验证重点是 DingTalk。

## 目录

- [通道现状](#通道现状)
- [DingTalk Stream 模式](#dingtalk-stream-模式)
- [最小配置](#最小配置)
- [Hub 启动后会发生什么](#hub-启动后会发生什么)
- [消息如何进入任务链路](#消息如何进入任务链路)
- [DingTalk 自然语言规划与本地校验](#dingtalk-自然语言规划与本地校验)
- [消息回传](#消息回传)
- [Webhook 路由说明](#webhook-路由说明)
- [与 LLM / 自定义模型服务商的关系](#与-llm--自定义模型服务商的关系)
- [测试验证](#测试验证)
- [下一步](#下一步)

---

## 通道现状

| 通道 | 当前文档状态 | 当前主运行时状态 |
|------|--------------|------------------|
| DingTalk | 主文档路径 | 已接入主链路 |
| Telegram | 模块存在 | 非当前主线文档重点 |
| Slack | 模块存在 | 非当前主线文档重点 |
| Discord | 模块存在 | 非当前主线文档重点 |
| WhatsApp | 模块存在 | 非当前主线文档重点 |
| Feishu / WeCom | 模块存在 | 非当前主线文档重点 |

如果你正在做当前主线验证，请优先关注 DingTalk。

---

## DingTalk Stream 模式

当前 `uhorse-hub` 使用 DingTalk 时，主推荐模式是：

- **Stream 模式**
- Hub 主动建立长连接
- 不依赖公网 IP 才能接收入站消息
- 不把 webhook 作为主入口说明

这也是当前仓库 README 与配置手册对齐后的推荐路径。

---

## 最小配置

在统一配置文件里启用 DingTalk：

```toml
[channels]
enabled = ["dingtalk"]

[channels.dingtalk]
app_key = "your_app_key"
app_secret = "your_app_secret"
agent_id = 123456789

[[channels.dingtalk.notification_bindings]]
node_id = "your-stable-node-id"
user_id = "your-dingtalk-user-id"

[[channels.dingtalk.skill_installers]]
user_id = "your-admin-user-id"
# staff_id = "your-staff-id"
# corp_id = "dingcorp-xxx"
```

> 注意：DingTalk 只能通过 **统一配置** 初始化。legacy `HubConfig` 模式不能初始化 DingTalk。

---

## Hub 启动后会发生什么

当 `channels.enabled` 包含 `dingtalk` 时，Hub 启动流程会：

1. 读取 `[channels.dingtalk]`
2. 初始化 `DingTalkChannel`
3. 以 Stream 模式启动 DingTalk 消息接收
4. 订阅入站消息
5. 把入站文本转成 Hub 任务
6. 在任务完成后按原会话回发结果

---

## 消息如何进入任务链路

当前主链路是：

```text
DingTalk inbound message
    → DingTalkChannel
    → submit_dingtalk_task(...)
    → plan_dingtalk_command(...)
    → Hub::submit_task(...)
    → 调度到在线 Node
    → Node 执行
    → Node 回传 TaskResult
    → Hub reply_task_result(...)
    → summarize_task_result(...)
    → 回发到原 DingTalk 会话
```

这意味着 DingTalk 消息不是在通道层本地直接处理，而是会先进入 LLM 规划，再进入 Hub-Node 任务执行链路。

此外，当前还提供一条**受控 Skill 安装薄入口**：

- `安装技能 <package> <download_url> [version]`
- `install skill <package> <download_url> [version]`

这条入口不会把文本交给自然语言任务规划，而是直接解析成 Skill 安装请求。

## 来源感知运行时

在当前 `v4.4.0` 主线口径下，通道消息进入 Hub 任务链路后，还会进入带来源元信息的 runtime 视图。

这里的目标不是把 DingTalk 变成 `memory / agent / skill` 管理入口，而是让运行时能够区分资源来自哪里、应该按什么边界共享或隔离。

可对外说明的最小语义是：

- `source_layer`：区分来源层级，例如全局、租户、用户等来源层。
- `source_scope`：区分来源作用域，用于表达共享 / 隔离边界。

这层语义服务于当前主线里的 source-aware runtime / UI 展示，不改变 DingTalk 作为通道入口与结果回传出口的职责。

---

## DingTalk 自然语言规划与本地校验

当前 `uhorse-hub` 不再把 DingTalk 文本限制为固定命令白名单，而是会：

1. 读取用户原始自然语言请求
2. 通过 LLM 规划单个 `Command`
3. 仅接受 `FileCommand`、`ShellCommand` 或受控 `BrowserCommand`
4. 对文件路径做本地校验，禁止绝对路径越界和 `..`
5. 对浏览器目标做本地校验，只允许公共 `http/https` URL，拒绝 `file://`、localhost、私网地址和其他越界目标
6. 拒绝危险 git，例如 `git reset --hard`、`git clean -fd`、`git push --force`

如果 LLM 返回非法 JSON、越界路径、非法浏览器目标或危险命令，Hub 会直接报错，不会下发到 Node。

对于 Skill 安装薄入口，当前边界还包括：

- 只接受 `skillhub` 来源
- 安装前会按 `channels.dingtalk.skill_installers` 校验发送者是否命中白名单
- 白名单可按 `user_id` / `staff_id` 命中，并可选叠加 `corp_id`
- 当前 DingTalk 文本入口只支持 install，不支持 refresh

---

## 消息回传

如果同时启用了 Node Desktop 本地通知镜像，当前主路径是由 Node Desktop 发起 pairing、用户在 DingTalk 中确认后写入运行时绑定；`channels.dingtalk.notification_bindings` 仅保留为兼容 seed/fallback，用于补充“节点通知 -> 钉钉用户”的回传路径。

当前结果回传逻辑会保留任务的完整执行结果，并按以下顺序尝试回到原 DingTalk 会话：

- 优先使用 `session_webhook`（未过期时）
- 群会话回退到 `conversation_id` 群消息发送
- 单聊回退到 `sender_user_id` 直接发送

回传内容策略可以概括为：

- 优先使用 LLM 基于 `CompletedTask` 生成自然语言总结
- 如果总结失败，则回退到结构化文本结果
- 任务规划或本地校验阶段失败时，会即时回发错误信息

当前主线已经用真实企业租户验证：不安全请求会即时错误回显，合法文件 / shell 请求与受控浏览器请求的执行结果都会原路回传到原会话。

因此 DingTalk 在当前主线中不只是“入站入口”，也是任务结果的回传出口。

---

## Webhook 路由说明

虽然当前主推荐是 Stream 模式，但 Hub 仍保留了兼容 / 辅助测试用 webhook 路由：

```text
GET  /api/v1/channels/dingtalk/webhook
POST /api/v1/channels/dingtalk/webhook
```

注意：

- 这不改变主推荐模式仍然是 Stream
- 文档与部署设计不应再把 webhook 当成默认入口

---

## 与 LLM / 自定义模型服务商的关系

DingTalk 与 LLM 都由 Hub 的统一配置驱动，因此二者通常一起出现在统一配置文件中。

例如：

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

当前代码支持自定义模型服务商：

- `provider` 可以写成任意自定义字符串
- 该 provider 会被当作 **Custom provider** 处理
- 当前客户端默认按 **OpenAI 兼容接口** 请求：
  - `POST {base_url}/chat/completions`
  - `Authorization: Bearer <api_key>`

所以如果你要在 DingTalk 入口后接入企业内部模型平台，只要该平台兼容这套接口即可。

---

## 测试验证

### 启动 Hub 并观察日志

```bash
./target/release/uhorse-hub --config hub.toml --log-level info
```

重点关注：

- DingTalk channel 是否初始化成功
- 是否以 Stream 模式启动
- 是否开始接收入站消息

### 配合 Node 验证本地链路

```bash
./target/release/uhorse-node --config node.toml --log-level info
curl http://127.0.0.1:8765/api/nodes
```

### 本地已验证的基础闭环测试

```bash
cargo test -p uhorse-hub test_local_hub_node_roundtrip_file_exists -- --nocapture
```

这条测试不依赖真实 DingTalk 凭据，但能证明 Hub-Node 任务执行主链路已闭合。

---

## 下一步

- [CONFIG.md](CONFIG.md)：统一配置与 legacy 配置边界
- [README.md](README.md)：项目总览
- [TESTING.md](TESTING.md)：测试与验证
- [deployments/DEPLOYMENT_V4.md](deployments/DEPLOYMENT_V4.md)：v4 部署说明

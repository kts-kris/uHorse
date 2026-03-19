# uHorse 部署指南

本文档说明当前仓库里 **部署相关资产的真实状态**，并给出面向当前主线的部署建议。

## 目录

- [当前结论](#当前结论)
- [仓库里的部署资产](#仓库里的部署资产)
- [推荐部署路径](#推荐部署路径)
- [不建议直接照搬的旧部署资产](#不建议直接照搬的旧部署资产)
- [部署前检查](#部署前检查)
- [下一步](#下一步)

---

## 当前结论

当前仓库的主线架构是 **v4.0 Hub-Node**。

因此如果你要做新部署，优先建议：

- Hub：部署 `uhorse-hub`
- Node：部署 `uhorse-node`
- DingTalk：按 **Stream 模式** 接入
- LLM：通过统一配置里的 `[llm]` 初始化

对应的主文档是：

- [DEPLOYMENT_V4.md](DEPLOYMENT_V4.md)

---

## 仓库里的部署资产

当前 `deployments/` 目录包含：

- `DEPLOYMENT.md`：本文件，说明部署资产状态
- `DEPLOYMENT_V4.md`：当前推荐的 v4.0 Hub-Node 部署指南
- `DISASTER_RECOVERY.md`：灾备文档
- `k8s/base/`：一组旧部署清单
- `prometheus/`：Prometheus 配置
- `grafana/`：Grafana 仪表板

---

## 推荐部署路径

### 本地或单机验证

先参考：

- [../INSTALL.md](../INSTALL.md)
- [../LOCAL_SETUP.md](../LOCAL_SETUP.md)
- [../CONFIG.md](../CONFIG.md)

### 当前主线生产 / 预生产路径

直接参考：

- [DEPLOYMENT_V4.md](DEPLOYMENT_V4.md)

它描述的是当前主线更贴近代码事实的部署口径：

- Hub 与 Node 分离
- Hub 对外提供 `/api/health`、`/api/nodes`、`/ws`
- Node 通过 `hub_url` 连接 Hub
- DingTalk 使用 Stream 模式
- LLM 支持内置 provider 与自定义 provider

---

## 不建议直接照搬的旧部署资产

仓库里现有的 `deployments/k8s/base/*` 仍然是旧单体 `uhorse` 视角，存在以下特征：

- `Deployment` 名称是 `uhorse`
- 容器启动对象默认指向旧单体运行形态
- 使用 `OPENCLAW_*` 环境变量
- 健康检查路径写的是 `/health/live` / `/health/ready`
- 配置仍按旧单体思路组织

这意味着：

- 它们可以作为历史参考
- 但 **不应直接作为当前 v4.0 Hub-Node 的生产部署模板**

如果你正在规划当前主线部署，请优先用 `DEPLOYMENT_V4.md` 里的思路，而不是直接套用 `k8s/base/*`。

---

## 部署前检查

### Hub

- [ ] 已准备 `uhorse-hub` 二进制
- [ ] 已确认使用统一配置还是 legacy `HubConfig`
- [ ] 如果启用 DingTalk，已准备 `channels.dingtalk`
- [ ] 如果启用 LLM，已准备 `[llm]`
- [ ] 如果使用自定义模型服务商，已确认目标服务兼容 OpenAI `/chat/completions`

### Node

- [ ] 已准备 `uhorse-node` 二进制
- [ ] `workspace_path` 已确认
- [ ] `connection.hub_url` 已确认
- [ ] Node 到 Hub 的网络可达

### 验证

- [ ] `curl http://<hub>/api/health`
- [ ] `curl http://<hub>/api/nodes`
- [ ] Node 成功连接 `/ws`

---

## 下一步

- [DEPLOYMENT_V4.md](DEPLOYMENT_V4.md)：当前主线部署方式
- [../CONFIG.md](../CONFIG.md)：配置结构说明
- [../CHANNELS.md](../CHANNELS.md)：DingTalk Stream 说明
- [../TESTING.md](../TESTING.md)：测试与验证命令

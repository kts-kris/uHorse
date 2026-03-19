# uHorse 部署目录说明

本目录包含与部署相关的文档和历史资产。

## 当前建议

如果你要部署 **当前主线 v4.0 Hub-Node 架构**，请优先看：

- [DEPLOYMENT_V4.md](DEPLOYMENT_V4.md)

如果你想了解本目录里各类部署文件的状态与适用边界，再看：

- [DEPLOYMENT.md](DEPLOYMENT.md)

---

## 目录结构

```text
deployments/
├── README.md
├── DEPLOYMENT.md
├── DEPLOYMENT_V4.md
├── DISASTER_RECOVERY.md
├── k8s/
│   └── base/
│       ├── deployment.yaml
│       ├── configmap.yaml
│       ├── secret.yaml
│       └── rbac.yaml
├── prometheus/
│   ├── prometheus.yml
│   └── alerts.yaml
└── grafana/
    └── uhorse-dashboard.json
```

---

## 文档索引

| 文档 | 用途 |
|------|------|
| [DEPLOYMENT_V4.md](DEPLOYMENT_V4.md) | 当前主线 Hub-Node 部署指南 |
| [DEPLOYMENT.md](DEPLOYMENT.md) | 部署资产现状、边界和迁移说明 |
| [DISASTER_RECOVERY.md](DISASTER_RECOVERY.md) | 灾备与恢复 |

---

## 当前资产状态

### 1. `DEPLOYMENT_V4.md`

这是当前最应该参考的部署文档，覆盖：

- `uhorse-hub`
- `uhorse-node`
- DingTalk Stream
- LLM 与自定义模型服务商
- Hub / Node 分离部署

### 2. `k8s/base/*`

这组文件仍然更偏向旧单体 `uhorse` 运行形态，主要特征包括：

- `OPENCLAW_*` 环境变量
- `/health/live` / `/health/ready`
- 单体 `uhorse` 部署对象

因此：

- 可以作为历史参考
- 不建议直接当作当前 v4.0 Hub-Node 生产模板

### 3. `prometheus/*` 与 `grafana/*`

这些监控资产仍可作为参考素材，但在接入当前主线部署前，建议先核对：

- 指标名称
- 抓取端口
- 健康检查路径
- 实际服务名称

---

## 推荐阅读顺序

1. [DEPLOYMENT_V4.md](DEPLOYMENT_V4.md)
2. [../CONFIG.md](../CONFIG.md)
3. [../CHANNELS.md](../CHANNELS.md)
4. [../TESTING.md](../TESTING.md)
5. [DEPLOYMENT.md](DEPLOYMENT.md)

---

## 说明

当前主线部署目标应默认理解为：

- Hub 负责 API、WebSocket、调度、DingTalk、结果回传
- Node 负责受控工作空间执行
- DingTalk 采用 Stream 模式
- LLM 支持内置 provider 和自定义 provider（OpenAI 兼容）

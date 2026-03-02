# uHorse 部署配置

本目录包含 uHorse 生产环境的完整部署配置和文档。

## 目录结构

```
deployments/
├── README.md                    # 本文件 - 部署配置说明
├── DEPLOYMENT.md                # 完整部署指南
├── DISASTER_RECOVERY.md         # 灾备方案文档
├── Dockerfile                   # 生产环境 Docker 镜像
├── docker-compose.yml           # 本地开发环境编排
├── k8s/                         # Kubernetes 部署配置
│   └── base/
│       ├── deployment.yaml      # Deployment, Service, HPA, PDB
│       ├── configmap.yaml       # ConfigMap 配置
│       ├── secret.yaml          # Secret 模板
│       └── rbac.yaml            # RBAC 权限配置
├── prometheus/                  # Prometheus 监控配置
│   ├── prometheus.yml           # Prometheus 主配置
│   └── alerts.yaml              # 告警规则
└── grafana/                     # Grafana 仪表板
    └── uhorse-dashboard.json  # 监控仪表板
```

## 快速开始

### 本地开发

```bash
# 使用 Docker Compose 启动完整环境
docker-compose up -d

# 查看服务状态
docker-compose ps

# 查看日志
docker-compose logs -f uhorse
```

### Kubernetes 部署

```bash
# 1. 创建 Secret
kubectl create secret generic uhorse-secrets \
  --from-literal=jwt_secret=$(openssl rand -hex 32) \
  --from-literal=telegram_bot_token=YOUR_TOKEN \
  -n uhorse

# 2. 部署应用
kubectl apply -f deployments/k8s/base/

# 3. 验证部署
kubectl get pods -n uhorse
```

## 文档索引

| 文档 | 说明 | 适用场景 |
|------|------|----------|
| [DEPLOYMENT.md](DEPLOYMENT.md) | 完整部署指南 | 首次部署、日常运维 |
| [DISASTER_RECOVERY.md](DISASTER_RECOVERY.md) | 灾备方案 | 灾难恢复、备份恢复 |

## 配置说明

### Dockerfile

多阶段构建，基于 `debian:bookworm-slim`：
- 构建阶段：使用 `rust:1.83-slim` 编译
- 运行阶段：最小化镜像，仅包含二进制文件
- 安全性：非 root 用户 (UID 1000) 运行
- 健康检查：30 秒间隔检查 `/health/live`

### docker-compose.yml

本地开发环境，包含：
- **uhorse**: 主应用
- **postgres**: PostgreSQL 14 数据库
- **redis**: Redis 7 缓存
- **prometheus**: 监控指标收集
- **grafana**: 可视化监控面板

### Kubernetes 配置

**deployment.yaml**
- Deployment: 3 副本，滚动更新
- Service: ClusterIP 和 LoadBalancer
- HPA: 3-10 副本自动扩缩容
- PDB: 最少 2 副本可用
- PVC: 数据卷和日志卷

**configmap.yaml**
- 环境变量配置
- 应用 TOML 配置文件

**secret.yaml**
- JWT 密钥
- 通道 Bot Token

**rbac.yaml**
- ServiceAccount
- Role (命名空间权限)
- ClusterRole (集群监控权限)

### Prometheus 配置

**prometheus.yml**
- 15 秒抓取间隔
- Kubernetes 服务发现
- uHorse 应用指标

**alerts.yaml**
- 5 个告警组
- 15+ 告警规则
- 覆盖可用性、资源、业务、数据库、安全

### Grafana 仪表板

**uhorse-dashboard.json**
- 19 个监控面板
- 实时服务状态
- API 性能指标
- 资源使用情况
- 业务指标统计

## 环境变量

### 必需配置

| 变量 | 说明 | 示例 |
|------|------|------|
| `OPENCLAW_JWT_SECRET` | JWT 签名密钥 (32 字符) | `openssl rand -hex 32` |
| `OPENCLAW_DATABASE_URL` | PostgreSQL 连接字符串 | `postgresql://user:pass@host:5432/db` |
| `OPENCLAW_REDIS_URL` | Redis 连接字符串 | `redis://host:6379` |

### 可选配置

| 变量 | 说明 | 示例 |
|------|------|------|
| `OPENCLAW_TELEGRAM_BOT_TOKEN` | Telegram Bot Token | `123456:ABC-DEF...` |
| `OPENCLAW_SLACK_BOT_TOKEN` | Slack Bot Token | `xoxb-...` |
| `OPENCLAW_DISCORD_BOT_TOKEN` | Discord Bot Token | `MTIzNDU2Nzg5MA...` |
| `RUST_LOG` | 日志级别 | `info`, `debug`, `warn` |

## 监控端点

| 端点 | 说明 | 格式 |
|------|------|------|
| `/health/live` | 存活性检查 | JSON |
| `/health/ready` | 就绪性检查 | JSON |
| `/metrics` | Prometheus 指标 | Prometheus 格式 |

## 资源限制

### 默认配置

| 资源 | Request | Limit |
|------|---------|-------|
| CPU | 100m | 500m |
| 内存 | 128Mi | 512Mi |

### 生产建议

| 资源 | Request | Limit |
|------|---------|-------|
| CPU | 500m | 2000m |
| 内存 | 512Mi | 2Gi |

## 高可用配置

### 副本数

- **最小**: 3 副本 (跨节点)
- **推荐**: 5 副本 (跨可用区)
- **HPA 范围**: 3-10 副本

### 数据库

- 主从复制 (1 主 2 从)
- 自动故障转移
- 连接池

### 缓存

- Redis Sentinel
- 3 节点集群
- 自动故障转移

## 备份策略

- **数据库**: 每日全量备份，保留 30 天
- **持久化卷**: 每日快照，保留 7 天
- **配置**: Git 版本控制

## 故障恢复

| 故障类型 | RTO | RPO |
|----------|-----|-----|
| Pod 故障 | < 5 分钟 | 0 |
| 节点故障 | < 10 分钟 | 0 |
| 数据库故障 | < 2 分钟 | < 1 分钟 |
| 可用区故障 | < 15 分钟 | < 5 分钟 |
| 完全灾难 | < 4 小时 | < 24 小时 |

## 安全建议

1. **更改默认密钥**: 生产环境必须更改 JWT_SECRET
2. **启用 TLS**: 使用 HTTPS 和 WSS
3. **网络策略**: 配置 Kubernetes NetworkPolicy
4. **镜像扫描**: 定期扫描镜像漏洞
5. **访问控制**: 使用 RBAC 最小权限

## 故障排查

### 常见问题

1. **Pod 无法启动**: 检查资源限制和镜像拉取
2. **数据库连接失败**: 检查 Service 和连接字符串
3. **内存不足**: 增加 memory limit
4. **CPU 飙升**: 检查是否有死循环或内存泄漏

### 日志查看

```bash
# 查看实时日志
kubectl logs -f deployment/uhorse -n uhorse

# 查看所有副本
kubectl logs -f -l app=uhorse -n uhorse --all-containers=true

# 查看最近日志
kubectl logs --tail=100 deployment/uhorse -n uhorse
```

## 升级指南

### 滚动更新

```bash
# 更新镜像
kubectl set image deployment/uhorse \
  uhorse=uhorse:v1.0.1 \
  -n uhorse

# 查看状态
kubectl rollout status deployment/uhorse -n uhorse

# 回滚
kubectl rollout undo deployment/uhorse -n uhorse
```

### 数据库迁移

```bash
# 执行迁移
kubectl exec deployment/uhorse -n uhorse -- \
  /app/uhorse migrate

# 回滚迁移
kubectl exec deployment/uhorse -n uhorse -- \
  /app/uhorse migrate rollback
```

## 更多信息

- [项目主文档](../README.md)
- [实施进度](../PROGRESS.md)
- [部署指南](DEPLOYMENT.md)
- [灾备方案](DISASTER_RECOVERY.md)

---

**维护者**: uHorse 团队
**版本**: v1.0.0
**最后更新**: 2026-03-02

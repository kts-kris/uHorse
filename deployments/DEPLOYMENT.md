# OpenClaw 部署指南

## 目录

- [概述](#概述)
- [前置要求](#前置要求)
- [本地开发环境](#本地开发环境)
- [Docker 部署](#docker-部署)
- [Kubernetes 部署](#kubernetes-部署)
- [监控配置](#监控配置)
- [验证测试](#验证测试)
- [常见问题](#常见问题)
- [升级流程](#升级流程)

## 概述

本文档提供 OpenClaw 的完整部署指南，包括本地开发环境、Docker 容器化部署和 Kubernetes 生产环境部署。

## 前置要求

### 硬件要求

**本地开发**
- CPU: 4 核心以上
- 内存: 8 GB 以上
- 磁盘: 20 GB 可用空间

**生产环境 (单副本)**
- CPU: 2 核心
- 内存: 512 MB
- 磁盘: 15 GB (数据 + 日志)

**生产环境 (推荐配置)**
- CPU: 4 核心
- 内存: 2 GB
- 磁盘: 50 GB SSD
- 副本数: 3+

### 软件要求

**构建环境**
```bash
# Rust 工具链
cargo 1.83+
rustc 1.83+

# 容器工具
Docker 24.0+
docker-compose 2.20+

# Kubernetes 工具 (生产部署)
kubectl 1.28+
helm 3.12+
```

**运行时依赖**
- PostgreSQL 14+
- Redis 7+

## 本地开发环境

### 1. 环境准备

```bash
# 克隆仓库
git clone https://github.com/openclaw/openclaw.git
cd openclaw

# 安装 Rust 工具链 (如果尚未安装)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### 2. 启动依赖服务

```bash
# 启动 PostgreSQL 和 Redis
docker-compose up -d postgres redis

# 等待服务就绪
docker-compose ps
```

### 3. 配置应用

```bash
# 复制配置模板
cp config.example.toml config.toml

# 编辑配置
vim config.toml
```

**config.toml 关键配置**
```toml
[server]
host = "127.0.0.1"
port = 8080

[database]
url = "postgresql://openclaw:password@localhost:5432/openclaw"

[channels.telegram]
bot_token = "YOUR_BOT_TOKEN"

[security.jwt]
secret = "CHANGE_ME_TO_RANDOM_32_CHAR_STRING"
```

### 4. 构建和运行

```bash
# 构建应用
cargo build --release

# 初始化数据库
./target/release/openclaw migrate

# 运行应用
./target/release/openclaw serve
```

### 5. 验证

```bash
# 健康检查
curl http://localhost:8080/health/live

# 查看指标
curl http://localhost:8080/metrics

# 测试 WebSocket
wscat -c ws://localhost:8080/ws
```

## Docker 部署

### 1. 构建镜像

```bash
# 构建生产镜像
docker build -t openclaw:latest .

# 验证镜像
docker images | grep openclaw
```

### 2. 使用 Docker Compose

```bash
# 启动完整环境
docker-compose up -d

# 查看日志
docker-compose logs -f openclaw

# 查看状态
docker-compose ps
```

### 3. 环境变量配置

创建 `.env` 文件：

```bash
# 服务配置
OPENCLAW_SERVER_HOST=0.0.0.0
OPENCLAW_SERVER_PORT=8080

# 数据库
OPENCLAW_DATABASE_URL=postgresql://openclaw:password@postgres:5432/openclaw

# Redis
OPENCLAW_REDIS_URL=redis://redis:6379

# 通道配置
OPENCLAW_TELEGRAM_BOT_TOKEN=your_bot_token_here

# 安全配置
OPENCLAW_JWT_SECRET=your_32_char_random_secret_here
```

### 4. 管理命令

```bash
# 停止服务
docker-compose down

# 重启服务
docker-compose restart

# 查看日志
docker-compose logs -f [service]

# 进入容器
docker-compose exec openclaw /bin/bash

# 数据库迁移
docker-compose exec openclaw /app/openclaw migrate

# 执行命令
docker-compose exec openclaw /app/openclaw --help
```

## Kubernetes 部署

### 1. 集群准备

```bash
# 验证集群连接
kubectl cluster-info
kubectl get nodes

# 创建命名空间
kubectl create namespace openclaw
```

### 2. 创建 Secret

```bash
# 生成 JWT 密钥
JWT_SECRET=$(openssl rand -hex 32)

# 创建 Secret
kubectl create secret generic openclaw-secrets \
  --from-literal=jwt_secret=${JWT_SECRET} \
  --from-literal=telegram_bot_token=YOUR_BOT_TOKEN \
  --namespace=openclaw

# 验证 Secret
kubectl get secret openclaw-secrets -n openclaw
```

### 3. 部署基础组件

```bash
# 部署 RBAC
kubectl apply -f deployments/k8s/base/rbac.yaml

# 部署 ConfigMap
kubectl apply -f deployments/k8s/base/configmap.yaml

# 部署 Secret (如果还没创建)
kubectl apply -f deployments/k8s/base/secret.yaml

# 部署应用
kubectl apply -f deployments/k8s/base/deployment.yaml
```

### 4. 验证部署

```bash
# 查看 Pod 状态
kubectl get pods -n openclaw

# 查看 Service
kubectl get svc -n openclaw

# 查看 PVC
kubectl get pvc -n openclaw

# 查看日志
kubectl logs -f deployment/openclaw -n openclaw
```

### 5. 访问服务

```bash
# 端口转发 (本地测试)
kubectl port-forward -n openclaw svc/openclaw 8080:8080

# 获取 LoadBalancer IP (生产环境)
kubectl get svc openclaw-lb -n openclaw
```

### 6. 水平扩缩容

```bash
# 手动扩容
kubectl scale deployment openclaw --replicas=5 -n openclaw

# 查看扩容状态
kubectl get pods -n openclaw -w

# 查看自动扩缩容状态
kubectl get hpa -n openclaw
```

### 7. 更新部署

```bash
# 构建新镜像
docker build -t openclaw:v1.0.1 .

# 推送到镜像仓库
docker tag openclaw:v1.0.1 registry.example.com/openclaw:v1.0.1
docker push registry.example.com/openclaw:v1.0.1

# 更新部署
kubectl set image deployment/openclaw \
  openclaw=registry.example.com/openclaw:v1.0.1 \
  -n openclaw

# 查看滚动更新状态
kubectl rollout status deployment/openclaw -n openclaw

# 回滚 (如果需要)
kubectl rollout undo deployment/openclaw -n openclaw
```

## 监控配置

### 1. Prometheus 部署

```bash
# 使用 Helm 安装 Prometheus
helm repo add prometheus-community \
  https://prometheus-community.github.io/helm-charts

helm repo update

helm install prometheus prometheus-community/kube-prometheus-stack \
  -n monitoring --create-namespace
```

### 2. 配置告警规则

```bash
# 应用告警规则
kubectl apply -f deployments/prometheus/alerts.yaml -n monitoring

# 应用 Prometheus 配置
kubectl apply -f deployments/prometheus/prometheus.yml -n monitoring
```

### 3. Grafana 仪表板

```bash
# 端口转发访问 Grafana
kubectl port-forward -n monitoring svc/prometheus-grafana 3000:80

# 登录 Grafana (默认凭据)
# URL: http://localhost:3000
# Username: admin
# Password: prom-operator

# 导入仪表板
# 1. 登录后点击 "+" → "Import"
# 2. 上传 deployments/grafana/openclaw-dashboard.json
# 3. 选择 Prometheus 数据源
# 4. 点击 "Import"
```

### 4. 告警通知配置

编辑 AlertManager ConfigMap:

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: alertmanager-config
  namespace: monitoring
data:
  alertmanager.yml: |
    global:
      resolve_timeout: 5m
    route:
      group_by: ['alertname', 'cluster', 'service']
      group_wait: 10s
      group_interval: 10s
      repeat_interval: 12h
      receiver: 'default'
      routes:
      - match:
          severity: critical
        receiver: 'pagerduty'
    receivers:
    - name: 'default'
      slack_configs:
      - api_url: 'YOUR_SLACK_WEBHOOK_URL'
        channel: '#alerts'
    - name: 'pagerduty'
      pagerduty_configs:
      - service_key: 'YOUR_PAGERDUTY_KEY'
```

## 验证测试

### 1. 健康检查

```bash
# 本地环境
curl http://localhost:8080/health/live
curl http://localhost:8080/health/ready

# Kubernetes 环境
kubectl exec -n openclaw deployment/openclaw -- \
  wget -qO- http://localhost:8080/health/live
```

### 2. API 测试

```bash
# WebSocket 连接测试
wscat -c ws://localhost:8080/ws

# 发送测试消息
echo '{"type":"ping","id":"test-001"}' | wscat -c ws://localhost:8080/ws

# 查看指标
curl http://localhost:8080/metrics | grep openclaw
```

### 3. 性能测试

```bash
# 使用 wrk 进行压力测试
wrk -t4 -c100 -d30s http://localhost:8080/health/live

# 查看资源使用
docker stats openclaw
```

### 4. 故障测试

```bash
# 模拟 Pod 故障
kubectl delete pod -l app=openclaw -n openclaw

# 验证自动恢复
kubectl get pods -n openclaw -w
```

## 常见问题

### 1. 数据库连接失败

**问题**: `Connection refused` 或 `Could not connect to database`

**解决**:
```bash
# 检查数据库状态
kubectl get pods -n openclaw | grep postgres

# 检查数据库连接字符串
kubectl get configmap openclaw-config -n openclaw -o yaml

# 测试数据库连接
kubectl run -it --rm psql --image=postgres:14 -- \
  psql postgresql://openclaw:password@postgres:5432/openclaw
```

### 2. 内存不足

**问题**: Pod 被 OOMKilled

**解决**:
```bash
# 增加内存限制
kubectl set resources deployment openclaw \
  --limits=memory=1Gi \
  --requests=memory=256Mi \
  -n openclaw
```

### 3. 镜像拉取失败

**问题**: `ImagePullBackOff` 或 `ErrImagePull`

**解决**:
```bash
# 验证镜像存在
docker images | grep openclaw

# 创建 imagePullSecret
kubectl create secret docker-registry regcred \
  --docker-server=registry.example.com \
  --docker-username=user \
  --docker-password=pass \
  -n openclaw

# 更新 Deployment 使用 imagePullSecret
kubectl patch deployment openclaw -n openclaw -p \
  '{"spec":{"template":{"spec":{"imagePullSecrets":[{"name":"regcred"}]}}}}'
```

### 4. 日志查看

```bash
# 实时查看 Pod 日志
kubectl logs -f deployment/openclaw -n openclaw

# 查看所有副本日志
kubectl logs -f -l app=openclaw -n openclaw --all-containers=true

# 查看最近的日志
kubectl logs --tail=100 deployment/openclaw -n openclaw
```

### 5. 进入容器调试

```bash
# 进入容器 Shell
kubectl exec -it deployment/openclaw -n openclaw -- /bin/bash

# 查看环境变量
kubectl exec deployment/openclaw -n openclaw -- env

# 测试网络连接
kubectl exec deployment/openclaw -n openclaw -- \
  wget -qO- http://postgres:5432
```

## 升级流程

### 1. 零停机升级

```bash
# 1. 构建新版本
docker build -t openclaw:v1.1.0 .

# 2. 推送到镜像仓库
docker push registry.example.com/openclaw:v1.1.0

# 3. 更新 Deployment
kubectl set image deployment/openclaw \
  openclaw=registry.example.com/openclaw:v1.1.0 \
  -n openclaw

# 4. 监控滚动更新
kubectl rollout status deployment/openclaw -n openclaw

# 5. 验证新版本
kubectl describe deployment openclaw -n openclaw
```

### 2. 数据库迁移

```bash
# 运行迁移
kubectl exec deployment/openclaw -n openclaw -- \
  /app/openclaw migrate

# 回滚迁移 (如果需要)
kubectl exec deployment/openclaw -n openclaw -- \
  /app/openclaw migrate rollback
```

### 3. 配置更新

```bash
# 更新 ConfigMap
kubectl apply -f deployments/k8s/base/configmap.yaml

# 触发 Pod 重启以加载新配置
kubectl rollout restart deployment/openclaw -n openclaw
```

### 4. 紧急回滚

```bash
# 查看更新历史
kubectl rollout history deployment/openclaw -n openclaw

# 回滚到上一版本
kubectl rollout undo deployment/openclaw -n openclaw

# 回滚到指定版本
kubectl rollout undo deployment/openclaw --to-revision=3 -n openclaw
```

## 生产环境检查清单

### 部署前

- [ ] 硬件资源满足要求
- [ ] 数据库已配置高可用
- [ ] Redis 已配置持久化
- [ ] TLS 证书已配置
- [ ] 域名已配置 DNS
- [ ] 监控告警已配置
- [ ] 日志收集已配置
- [ ] 备份策略已配置

### 部署中

- [ ] Secret 已创建
- [ ] ConfigMap 已配置
- [ ] RBAC 已配置
- [ ] PVC 已创建并绑定
- [ ] Pod 全部 Running
- [ ] Service 可访问
- [ ] 健康检查通过

### 部署后

- [ ] 监控指标正常
- [ ] 日志无错误
- [ ] 性能测试通过
- [ ] 故障切换测试通过
- [ ] 备份测试通过
- [ ] 文档已更新
- [ ] 运维团队已培训

---

**文档版本**: v1.0.0
**最后更新**: 2026-03-02
**维护者**: OpenClaw 团队

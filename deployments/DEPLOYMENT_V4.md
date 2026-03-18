# uHorse v4.0 Hub-Node 部署指南

## 概述

uHorse v4.0 采用 **Hub-Node 分布式架构**：
- **Hub (云端中枢)**: 部署在云服务器，负责 API 网关、任务调度、节点管理
- **Node (本地节点)**: 部署在员工电脑，负责本地命令执行、工作空间管理

```
┌─────────────────┐                      ┌─────────────────┐
│     Hub         │◄──── WebSocket ────►│     Node        │
│  (云端中枢)     │                      │   (本地节点)    │
│                 │                      │                 │
│  • API 网关     │                      │  • 文件操作     │
│  • 任务调度     │                      │  • Shell 执行   │
│  • 节点管理     │                      │  • 数据库访问   │
│  • 会话管理     │                      │  • 浏览器控制   │
└─────────────────┘                      └─────────────────┘
        │                                        │
        ▼                                        ▼
┌─────────────────┐                      ┌─────────────────┐
│   PostgreSQL    │                      │   Workspace     │
│   Redis         │                      │   工作目录      │
└─────────────────┘                      └─────────────────┘
```

---

## 下载二进制

从 GitHub Release 下载对应平台的二进制：

```bash
# macOS Apple Silicon (M1/M2/M3)
wget https://github.com/kts-kris/uHorse/releases/download/v4.0.0-alpha.2/uhorse-hub-4.0.0-alpha.2-aarch64-apple-darwin.tar.gz
wget https://github.com/kts-kris/uHorse/releases/download/v4.0.0-alpha.2/uhorse-node-4.0.0-alpha.2-aarch64-apple-darwin.tar.gz

# macOS Intel
wget https://github.com/kts-kris/uHorse/releases/download/v4.0.0-alpha.2/uhorse-hub-4.0.0-alpha.2-x86_64-apple-darwin.tar.gz
wget https://github.com/kts-kris/uHorse/releases/download/v4.0.0-alpha.2/uhorse-node-4.0.0-alpha.2-x86_64-apple-darwin.tar.gz

# Linux x86_64
wget https://github.com/kts-kris/uHorse/releases/download/v4.0.0-alpha.2/uhorse-hub-4.0.0-alpha.2-x86_64-unknown-linux-gnu.tar.gz
wget https://github.com/kts-kris/uHorse/releases/download/v4.0.0-alpha.2/uhorse-node-4.0.0-alpha.2-x86_64-unknown-linux-gnu.tar.gz

# Windows x86_64
wget https://github.com/kts-kris/uHorse/releases/download/v4.0.0-alpha.2/uhorse-hub-4.0.0-alpha.2-x86_64-pc-windows-msvc.zip
wget https://github.com/kts-kris/uHorse/releases/download/v4.0.0-alpha.2/uhorse-node-4.0.0-alpha.2-x86_64-pc-windows-msvc.zip
```

---

## Hub 部署 (云端)

### 1. 解压并配置

```bash
# 解压
tar -xzf uhorse-hub-*.tar.gz
cd uhorse-hub

# 查看帮助
./uhorse-hub --help
```

### 2. 创建配置文件

创建 `hub.toml`：

```toml
# Hub 配置

[server]
# 监听地址 (0.0.0.0 表示所有接口)
host = "0.0.0.0"
# HTTP API 端口
http_port = 8080
# WebSocket 端口 (Node 连接)
ws_port = 8081

[database]
# PostgreSQL 连接
url = "postgresql://uhorse:password@localhost:5432/uhorse"
# 连接池大小
pool_size = 10

[redis]
# Redis 连接
url = "redis://localhost:6379"

[security]
# JWT 密钥 (生成: openssl rand -hex 32)
jwt_secret = "your-64-char-hex-secret-here"
# Token 过期时间 (秒)
token_expiry = 86400

[hub]
# 节点心跳超时 (秒)
node_timeout = 120
# 任务重试次数
task_retry_count = 3
```

### 3. 启动 Hub

```bash
# 前台运行
./uhorse-hub --config hub.toml

# 后台运行
nohup ./uhorse-hub --config hub.toml > hub.log 2>&1 &

# 使用 systemd (推荐)
sudo cp uhorse-hub.service /etc/systemd/system/
sudo systemctl enable uhorse-hub
sudo systemctl start uhorse-hub
```

### 4. 验证 Hub

```bash
# 健康检查
curl http://localhost:8080/health/live
curl http://localhost:8080/health/ready

# 查看指标
curl http://localhost:8080/metrics

# 检查 WebSocket
wscat -c ws://localhost:8081/ws
```

---

## Node 部署 (本地)

### 1. 解压并配置

```bash
# 解压
tar -xzf uhorse-node-*.tar.gz
cd uhorse-node

# 查看帮助
./uhorse-node --help
```

### 2. 创建配置文件

创建 `node.toml`：

```toml
# Node 配置

[node]
# 节点名称 (唯一标识)
name = "employee-laptop-001"
# 工作空间路径 (授权的目录)
workspace_path = "/Users/username/projects"

[connection]
# Hub WebSocket 地址
hub_url = "wss://hub.yourcompany.com:8081/ws"
# 重连间隔 (秒)
reconnect_interval = 5
# 心跳间隔 (秒)
heartbeat_interval = 30

[security]
# 节点认证 Token (从 Hub 管理界面获取)
auth_token = "your-node-token-here"

[permissions]
# 允许的操作
allow_file_read = true
allow_file_write = true
allow_shell_execute = false  # 生产环境建议关闭
allow_database = true
allow_browser = false

# 禁止访问的路径模式
denied_patterns = [
    "**/.env",
    "**/secrets/**",
    "**/.ssh/**",
    "**/credentials.*"
]
```

### 3. 启动 Node

```bash
# 前台运行
./uhorse-node --config node.toml

# 后台运行
nohup ./uhorse-node --config node.toml > node.log 2>&1 &

# macOS launchd (开机自启)
cp com.uhorse.node.plist ~/Library/LaunchAgents/
launchctl load ~/Library/LaunchAgents/com.uhorse.node.plist
```

### 4. 验证 Node

```bash
# 查看日志
tail -f node.log

# 应看到类似输出:
# [INFO] Node started: employee-laptop-001
# [INFO] Connected to Hub: wss://hub.yourcompany.com:8081/ws
# [INFO] Workspace: /Users/username/projects
```

---

## Docker 部署

### Hub Docker

```dockerfile
# Dockerfile.hub
FROM rust:1.83-slim as builder
WORKDIR /app
COPY . .
RUN cargo build --release -p uhorse-hub

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/uhorse-hub /usr/local/bin/
EXPOSE 8080 8081
CMD ["uhorse-hub"]
```

```bash
# 构建
docker build -f Dockerfile.hub -t uhorse-hub:4.0 .

# 运行
docker run -d \
  --name uhorse-hub \
  -p 8080:8080 -p 8081:8081 \
  -v $(pwd)/hub.toml:/etc/uhorse/hub.toml \
  uhorse-hub:4.0
```

### Node Docker (可选)

```dockerfile
# Dockerfile.node
FROM rust:1.83-slim as builder
WORKDIR /app
COPY . .
RUN cargo build --release -p uhorse-node

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/uhorse-node /usr/local/bin/
CMD ["uhorse-node"]
```

---

## Kubernetes 部署 (Hub)

### 1. Namespace 和 Secret

```yaml
# namespace.yaml
apiVersion: v1
kind: Namespace
metadata:
  name: uhorse
```

```yaml
# secret.yaml
apiVersion: v1
kind: Secret
metadata:
  name: uhorse-secrets
  namespace: uhorse
type: Opaque
stringData:
  jwt_secret: "your-64-char-hex-secret"
  database_url: "postgresql://uhorse:password@postgres:5432/uhorse"
  redis_url: "redis://redis:6379"
```

### 2. Hub Deployment

```yaml
# hub-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: uhorse-hub
  namespace: uhorse
spec:
  replicas: 3
  selector:
    matchLabels:
      app: uhorse-hub
  template:
    metadata:
      labels:
        app: uhorse-hub
    spec:
      containers:
      - name: hub
        image: ghcr.io/kts-kris/uhorse/hub:4.0.0
        ports:
        - containerPort: 8080
          name: http
        - containerPort: 8081
          name: websocket
        env:
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: uhorse-secrets
              key: database_url
        - name: REDIS_URL
          valueFrom:
            secretKeyRef:
              name: uhorse-secrets
              key: redis_url
        - name: JWT_SECRET
          valueFrom:
            secretKeyRef:
              name: uhorse-secrets
              key: jwt_secret
        resources:
          requests:
            memory: "256Mi"
            cpu: "250m"
          limits:
            memory: "512Mi"
            cpu: "500m"
        livenessProbe:
          httpGet:
            path: /health/live
            port: 8080
          initialDelaySeconds: 10
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /health/ready
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 5
```

### 3. Service

```yaml
# hub-service.yaml
apiVersion: v1
kind: Service
metadata:
  name: uhorse-hub
  namespace: uhorse
spec:
  selector:
    app: uhorse-hub
  ports:
  - name: http
    port: 8080
    targetPort: 8080
  - name: websocket
    port: 8081
    targetPort: 8081
  type: ClusterIP

---
apiVersion: v1
kind: Service
metadata:
  name: uhorse-hub-lb
  namespace: uhorse
spec:
  selector:
    app: uhorse-hub
  ports:
  - name: http
    port: 80
    targetPort: 8080
  - name: websocket
    port: 8081
    targetPort: 8081
  type: LoadBalancer
```

### 4. 部署

```bash
# 应用配置
kubectl apply -f namespace.yaml
kubectl apply -f secret.yaml
kubectl apply -f hub-deployment.yaml
kubectl apply -f hub-service.yaml

# 验证
kubectl get pods -n uhorse
kubectl get svc -n uhorse
```

---

## 安全配置

### 1. TLS 证书

```yaml
# tls-cert.yaml
apiVersion: cert-manager.io/v1
kind: Certificate
metadata:
  name: uhorse-hub-cert
  namespace: uhorse
spec:
  secretName: uhorse-hub-tls
  issuerRef:
    name: letsencrypt-prod
    kind: ClusterIssuer
  dnsNames:
  - hub.yourcompany.com
```

### 2. Ingress

```yaml
# ingress.yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: uhorse-hub
  namespace: uhorse
  annotations:
    nginx.ingress.kubernetes.io/websocket-services: uhorse-hub
spec:
  ingressClassName: nginx
  tls:
  - hosts:
    - hub.yourcompany.com
    secretName: uhorse-hub-tls
  rules:
  - host: hub.yourcompany.com
    http:
      paths:
      - path: /
        pathType: Prefix
        backend:
          service:
            name: uhorse-hub
            port:
              number: 8080
      - path: /ws
        pathType: Prefix
        backend:
          service:
            name: uhorse-hub
            port:
              number: 8081
```

---

## 监控配置

### Prometheus ServiceMonitor

```yaml
apiVersion: monitoring.coreos.com/v1
kind: ServiceMonitor
metadata:
  name: uhorse-hub
  namespace: uhorse
spec:
  selector:
    matchLabels:
      app: uhorse-hub
  endpoints:
  - port: http
    path: /metrics
    interval: 30s
```

### 关键指标

| 指标 | 说明 | 告警阈值 |
|------|------|----------|
| `uhorse_connected_nodes` | 连接的节点数 | < 1 |
| `uhorse_active_tasks` | 活跃任务数 | > 100 |
| `uhorse_task_duration_seconds` | 任务执行时间 | p99 > 30s |
| `uhorse_websocket_connections` | WebSocket 连接数 | - |

---

## 故障排查

### Node 连接失败

```bash
# 检查 Node 日志
tail -f node.log | grep -i "error\|failed"

# 检查网络连通性
ping hub.yourcompany.com
telnet hub.yourcompany.com 8081

# 检查 TLS 证书
openssl s_client -connect hub.yourcompany.com:8081

# 检查认证 Token
# 确保 Token 有效且未过期
```

### Hub 健康检查失败

```bash
# 检查 Hub 日志
kubectl logs -f deployment/uhorse-hub -n uhorse

# 检查数据库连接
kubectl exec -it deployment/uhorse-hub -n uhorse -- \
  psql $DATABASE_URL -c "SELECT 1"

# 检查 Redis 连接
kubectl exec -it deployment/uhorse-hub -n uhorse -- \
  redis-cli -u $REDIS_URL ping
```

---

## 版本升级

### 升级 Hub

```bash
# 1. 下载新版本
wget https://github.com/kts-kris/uHorse/releases/download/v4.0.0/uhorse-hub-4.0.0-*.tar.gz

# 2. 备份配置
cp hub.toml hub.toml.bak

# 3. 停止旧版本
sudo systemctl stop uhorse-hub

# 4. 替换二进制
tar -xzf uhorse-hub-*.tar.gz
sudo cp uhorse-hub /usr/local/bin/

# 5. 启动新版本
sudo systemctl start uhorse-hub

# 6. 验证
sudo systemctl status uhorse-hub
curl http://localhost:8080/health/ready
```

### 升级 Node

```bash
# 1. 下载新版本
wget https://github.com/kts-kris/uHorse/releases/download/v4.0.0/uhorse-node-4.0.0-*.tar.gz

# 2. 停止旧版本
pkill uhorse-node

# 3. 替换二进制
tar -xzf uhorse-node-*.tar.gz

# 4. 启动新版本
./uhorse-node --config node.toml
```

---

## 检查清单

### Hub 部署检查

- [ ] PostgreSQL 已配置并可访问
- [ ] Redis 已配置并可访问
- [ ] TLS 证书已配置
- [ ] JWT 密钥已生成 (64 字符 hex)
- [ ] 健康检查通过 (`/health/ready`)
- [ ] WebSocket 端口可访问
- [ ] 监控指标正常

### Node 部署检查

- [ ] 工作空间路径正确
- [ ] Hub URL 可访问
- [ ] 认证 Token 有效
- [ ] 权限配置符合安全要求
- [ ] 禁止模式已配置
- [ ] 日志显示连接成功

---

**文档版本**: v4.0.0
**最后更新**: 2026-03-18
**维护者**: uHorse 团队

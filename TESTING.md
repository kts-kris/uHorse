# OpenClaw 测试指南

## 目录

- [快速测试](#快速测试)
- [本地开发测试](#本地开发测试)
- [Docker 测试](#docker-测试)
- [Kubernetes 测试](#kubernetes-测试)
- [功能测试](#功能测试)
- [性能测试](#性能测试)
- [集成测试](#集成测试)

---

## 快速测试

### 1. 一键启动 (Docker Compose)

```bash
# 启动完整环境
docker-compose up -d

# 等待服务就绪
sleep 10

# 健康检查
curl http://localhost:8080/health/live

# 查看指标
curl http://localhost:8080/metrics
```

### 2. 快速验证

```bash
# 运行完整测试脚本
bash scripts/test.sh
```

---

## 本地开发测试

### 1. 环境准备

```bash
# 安装依赖
cargo install cargo-watch cargo-nextest

# 启动依赖服务
docker-compose up -d postgres redis

# 等待服务就绪
docker-compose ps
```

### 2. 编译和运行

```bash
# 开发模式运行 (热重载)
cargo watch -x run

# 或直接运行
cargo run --release

# 后台运行
cargo run --release > openclaw.log 2>&1 &
```

### 3. 单元测试

```bash
# 运行所有测试
cargo test

# 运行特定模块测试
cargo test --package openclaw-core
cargo test --package openclaw-gateway

# 带输出的测试
cargo test -- --nocapture

# 运行未通过的测试
cargo test -- --ignored
```

### 4. 集成测试

```bash
# 运行集成测试
cargo test --test '*'

# 指定测试线程数
cargo test -- --test-threads=1
```

---

## Docker 测试

### 1. 构建镜像

```bash
# 构建生产镜像
docker build -t openclaw:latest .

# 验证镜像
docker images | grep openclaw

# 检查镜像大小
docker inspect openclaw:latest | grep Size
```

### 2. 运行容器

```bash
# 单独运行应用容器
docker run -d \
  --name openclaw \
  -p 8080:8080 \
  -p 9090:9090 \
  -e OPENCLAW_DATABASE_URL="postgresql://openclaw:password@host.docker.internal:5432/openclaw" \
  -e OPENCLAW_REDIS_URL="redis://host.docker.internal:6379" \
  -e OPENCLAW_JWT_SECRET="test-secret-for-development-only" \
  openclaw:latest

# 查看日志
docker logs -f openclaw

# 进入容器
docker exec -it openclaw /bin/bash
```

### 3. Docker Compose 测试

```bash
# 启动所有服务
docker-compose up -d

# 查看服务状态
docker-compose ps

# 查看日志
docker-compose logs -f

# 重启服务
docker-compose restart openclaw

# 停止服务
docker-compose down

# 清理数据
docker-compose down -v
```

### 4. 健康检查

```bash
# 存活性检查
curl http://localhost:8080/health/live
# 预期输出: {"status":"healthy"}

# 就绪性检查
curl http://localhost:8080/health/ready
# 预期输出: {"status":"ready"}

# 查看版本
curl http://localhost:8080/health/live | jq .
```

---

## Kubernetes 测试

### 1. 本地 Kubernetes (Kind/MicroK8s)

```bash
# 使用 Kind 创建本地集群
kind create cluster --name openclaw-test

# 或使用 MicroK8s
microk8s install
microk8s start
microk8s enable dns storage ingress
```

### 2. 部署到 K8s

```bash
# 创建命名空间
kubectl create namespace openclaw

# 生成测试密钥
JWT_SECRET=$(openssl rand -hex 32)

# 创建 Secret
kubectl create secret generic openclaw-secrets \
  --from-literal=jwt_secret=${JWT_SECRET} \
  --from-literal=telegram_bot_token="" \
  --namespace=openclaw

# 部署应用
kubectl apply -f deployments/k8s/base/rbac.yaml
kubectl apply -f deployments/k8s/base/configmap.yaml
kubectl apply -f deployments/k8s/base/secret.yaml
kubectl apply -f deployments/k8s/base/deployment.yaml
```

### 3. 验证部署

```bash
# 查看 Pod 状态
kubectl get pods -n openclaw

# 等待 Pod 就绪
kubectl wait --for=condition=ready pod -l app=openclaw -n openclaw --timeout=60s

# 查看 Service
kubectl get svc -n openclaw

# 查看 PVC
kubectl get pvc -n openclaw

# 查看日志
kubectl logs -f deployment/openclaw -n openclaw
```

### 4. 端口转发测试

```bash
# 本地访问
kubectl port-forward -n openclaw svc/openclaw 8080:8080

# 测试健康检查
curl http://localhost:8080/health/live
```

### 5. 扩缩容测试

```bash
# 手动扩容
kubectl scale deployment openclaw --replicas=5 -n openclaw

# 观察扩容
kubectl get pods -n openclaw -w

# 查看自动扩缩容
kubectl get hpa -n openclaw
```

---

## 功能测试

### 1. WebSocket 连接测试

```bash
# 安装 wscat
npm install -g wscat

# 连接 WebSocket
wscat -c ws://localhost:8080/ws

# 发送 ping 消息
> {"type":"ping","id":"test-001"}
< {"type":"pong","id":"test-001"}

# 发送认证消息
> {"type":"auth","token":"your_jwt_token"}
< {"type":"auth_result","success":true}
```

### 2. HTTP API 测试

```bash
# 健康检查
curl http://localhost:8080/health/live

# 获取指标
curl http://localhost:8080/metrics

# 获取版本信息
curl http://localhost:8080/health/live | jq '.version'

# 测试 404
curl http://localhost:8080/not-found
```

### 3. 通道集成测试

#### Telegram 测试

```bash
# 发送测试消息 (需要 Bot Token)
curl -X POST http://localhost:8080/api/v1/telegram/test \
  -H "Content-Type: application/json" \
  -d '{"chat_id":"YOUR_CHAT_ID","text":"Test message"}'
```

#### Slack 测试

```bash
# 验证 Webhook
curl -X POST http://localhost:8080/api/v1/slack/webhook \
  -H "Content-Type: application/json" \
  -d '{"token":"YOUR_VERIFICATION_TOKEN","challenge":"test_challenge","type":"url_verification"}'
```

### 4. 工具执行测试

```bash
# 计算器工具
curl -X POST http://localhost:8080/api/v1/tools/execute \
  -H "Content-Type: application/json" \
  -d '{"tool":"calculator","expression":"2+2"}'

# HTTP 请求工具
curl -X POST http://localhost:8080/api/v1/tools/execute \
  -H "Content-Type: application/json" \
  -d '{"tool":"http","url":"https://api.github.com","method":"GET"}'

# 搜索工具
curl -X POST http://localhost:8080/api/v1/tools/execute \
  -H "Content-Type: application/json" \
  -d '{"tool":"search","query":"Rust programming"}'
```

### 5. 认证测试

```bash
# 获取访问令牌
curl -X POST http://localhost:8080/api/v1/auth/token \
  -H "Content-Type: application/json" \
  -d '{"device_id":"test-device","pairing_code":"123456"}'

# 使用令牌访问受保护资源
curl http://localhost:8080/api/v1/sessions \
  -H "Authorization: Bearer YOUR_TOKEN"
```

---

## 性能测试

### 1. 基准测试

```bash
# 使用 wrk 进行压力测试
wrk -t4 -c100 -d30s http://localhost:8080/health/live

# 使用 ab (Apache Bench)
ab -n 10000 -c 100 http://localhost:8080/health/live

# 使用 hey
hey -n 10000 -c 100 http://localhost:8080/health/live
```

### 2. 延迟测试

```bash
# 安装 bombardier
go install github.com/codesenberg/bombardier@latest

# 运行延迟测试
bombardier -l -c 10 -d 30s http://localhost:8080/health/live

# WebSocket 延迟测试
wscat -c ws://localhost:8080/ws
# 测量 ping-pong 延迟
```

### 3. 内存测试

```bash
# 监控内存使用
docker stats openclaw

# Kubernetes 环境内存使用
kubectl top pod -n openclaw

# 内存泄漏检测
# 运行长时间测试
wrk -t4 -c50 -d300s http://localhost:8080/
# 观察内存是否持续增长
```

### 4. 并发测试

```bash
# 逐步增加并发
for c in 10 50 100 500 1000; do
  echo "Testing with $c concurrent connections"
  wrk -t4 -c$c -d30s http://localhost:8080/health/live
  sleep 5
done
```

---

## 集成测试

### 1. 端到端测试脚本

创建 `scripts/test-e2e.sh`:

```bash
#!/bin/bash
set -e

echo "=== OpenClaw E2E Test ==="

# 1. 健康检查
echo "Testing health endpoint..."
curl -f http://localhost:8080/health/live || exit 1

# 2. WebSocket 连接
echo "Testing WebSocket connection..."
timeout 5 wscat -c ws://localhost:8080/ws <<< '{"type":"ping","id":"test"}' || exit 1

# 3. 指标端点
echo "Testing metrics endpoint..."
curl -f http://localhost:8080/metrics | grep openclaw || exit 1

# 4. 数据库连接
echo "Testing database connection..."
kubectl exec -n openclaw deployment/openclaw -- \
  /app/openclaw db:ping || exit 1

echo "=== All tests passed! ==="
```

### 2. 故障注入测试

```bash
# 测试 Pod 故障恢复
kubectl delete pod -l app=openclaw -n openclaw
# 验证自动重建
kubectl get pods -n openclaw -w

# 测试节点故障
kubectl cordon node-1
kubectl drain node-1 --ignore-daemonsets --delete-emptydir-data
# 验证 Pod 迁移

# 测试资源限制
# 触发 OOM
kubectl run -it --rm stress --image=progrium/stress -- \
  stress --vm 1 --vm-bytes 500M --timeout 30s
```

### 3. 监控集成测试

```bash
# 访问 Prometheus
kubectl port-forward -n monitoring svc/prometheus-kube-prometheus-prometheus 9090:9090
# 浏览器打开 http://localhost:9090

# 查询指标
curl -G http://localhost:9090/api/v1/query \
  --data-urlencode 'query=up{job="openclaw"}'

# 访问 Grafana
kubectl port-forward -n monitoring svc/prometheus-grafana 3000:80
# 浏览器打开 http://localhost:3000 (admin/prom-operator)

# 验证告警
kubectl port-forward -n monitoring svc/prometheus-kube-prometheus-alertmanager 9093:9093
# 浏览器打开 http://localhost:9093
```

---

## 自动化测试脚本

### 完整测试脚本

创建 `scripts/test.sh`:

```bash
#!/bin/bash
set -e

echo "╔════════════════════════════════════════════════╗"
echo "║     OpenClaw 自动化测试套件                   ║"
echo "╚════════════════════════════════════════════════╝"

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

pass() { echo -e "${GREEN}✓${NC} $1"; }
fail() { echo -e "${RED}✗${NC} $1"; exit 1; }
info() { echo -e "${YELLOW}→${NC} $1"; }

# 测试环境检查
info "检查测试环境..."
command -v docker >/dev/null 2>&1 || fail "Docker 未安装"
command -v curl >/dev/null 2>&1 || fail "curl 未安装"
command -v jq >/dev/null 2>&1 || fail "jq 未安装"
pass "环境检查通过"

# 编译测试
info "编译项目..."
cargo build --release 2>&1 | tail -n 5
pass "编译成功"

# 单元测试
info "运行单元测试..."
cargo test --quiet 2>&1 | tail -n 5
pass "单元测试通过"

# Docker 构建测试
info "构建 Docker 镜像..."
docker build -t openclaw:test -f Dockerfile . >/dev/null 2>&1
pass "Docker 镜像构建成功"

# 启动测试环境
info "启动测试环境..."
docker-compose up -d postgres redis openclaw >/dev/null 2>&1
sleep 10
pass "测试环境启动成功"

# 健康检查
info "执行健康检查..."
HEALTH=$(curl -s http://localhost:8080/health/live)
echo "$HEALTH" | jq -e '.status == "healthy"' >/dev/null 2>&1
pass "健康检查通过"

# 就绪检查
info "执行就绪检查..."
READY=$(curl -s http://localhost:8080/health/ready)
echo "$READY" | jq -e '.status == "ready"' >/dev/null 2>&1
pass "就绪检查通过"

# 指标测试
info "验证指标端点..."
METRICS=$(curl -s http://localhost:8080/metrics)
echo "$METRICS" | grep -q "openclaw_"
pass "指标端点正常"

# WebSocket 测试
info "测试 WebSocket 连接..."
if command -v wscat >/dev/null 2>&1; then
    timeout 3 wscat -c ws://localhost:8080/ws <<< '{"type":"ping","id":"test"}' | grep -q "pong"
    pass "WebSocket 连接正常"
else
    info "wscat 未安装，跳过 WebSocket 测试"
fi

# 性能测试
info "执行性能测试..."
if command -v wrk >/dev/null 2>&1; then
    RESULT=$(wrk -t2 -c10 -d5s http://localhost:8080/health/live 2>&1)
    echo "$RESULT" | grep -q "Requests/sec"
    pass "性能测试完成"
else
    info "wrk 未安装，跳过性能测试"
fi

# 清理
info "清理测试环境..."
docker-compose down -v >/dev/null 2>&1
pass "清理完成"

echo ""
echo "╔════════════════════════════════════════════════╗"
echo "║     所有测试通过 ✓                            ║"
echo "╚════════════════════════════════════════════════╝"
```

### 运行测试

```bash
# 赋予执行权限
chmod +x scripts/test.sh

# 运行完整测试
./scripts/test.sh
```

---

## 测试检查清单

### 部署前

- [ ] 代码编译通过
- [ ] 单元测试通过
- [ ] 代码格式检查
- [ ] Clippy 检查

### 部署后

- [ ] 健康检查通过
- [ ] 就绪检查通过
- [ ] 指标端点可访问
- [ ] WebSocket 可连接
- [ ] 日志无错误

### 功能测试

- [ ] API 响应正常
- [ ] 通道集成正常
- [ ] 工具执行正常
- [ ] 认证授权正常
- [ ] 数据库操作正常

### 性能测试

- [ ] 并发请求正常
- [ ] 响应延迟可接受
- [ ] 内存使用稳定
- [ ] 无内存泄漏

### 高可用测试

- [ ] Pod 故障自动恢复
- [ ] 节点故障自动迁移
- [ ] 滚动更新无停机
- [ ] 自动扩缩容正常

---

## 常见问题

### 1. 测试环境无法启动

```bash
# 检查端口占用
lsof -i :8080

# 清理旧容器
docker rm -f $(docker ps -aq)

# 清理旧网络
docker network prune
```

### 2. 数据库连接失败

```bash
# 检查数据库状态
docker-compose ps postgres

# 查看数据库日志
docker-compose logs postgres

# 重启数据库
docker-compose restart postgres
```

### 3. 单元测试失败

```bash
# 带输出运行测试
cargo test -- --nocapture

# 运行特定测试
cargo test test_name

# 查看详细错误
RUST_BACKTRACE=1 cargo test
```

---

## 更多信息

- [部署指南](deployments/DEPLOYMENT.md)
- [灾备方案](deployments/DISASTER_RECOVERY.md)
- [项目进度](PROGRESS.md)

---

**文档版本**: v1.0.0
**最后更新**: 2026-03-02

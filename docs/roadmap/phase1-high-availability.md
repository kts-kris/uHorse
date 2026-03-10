# Phase 1: 高可用性基础设施

## 概述

**目标**: 构建服务发现、负载均衡、分布式配置能力，实现 3 节点集群自动故障转移 < 30s

**周期**: 4 周

**状态**: ✅ 已完成

---

## 1. 服务发现与注册

### 1.1 功能需求

- 支持多后端: etcd (主) / Consul (备)
- 服务自动注册/注销
- 健康检查 (心跳 + TTL)
- 服务元数据管理

### 1.2 技术设计

```rust
// ServiceRegistry trait
#[async_trait]
pub trait ServiceRegistry: Send + Sync {
    async fn register(&self, instance: &ServiceInstance, options: &RegistrationOptions) -> Result<()>;
    async fn deregister(&self, service_name: &str, instance_id: &str) -> Result<()>;
    async fn discover(&self, service_name: &str) -> Result<Vec<ServiceInstance>>;
    async fn watch(&self, service_name: &str) -> Result<ServiceWatchStream>;
}
```

### 1.3 实现文件

| 文件 | 功能 | 状态 |
|------|------|------|
| `uhorse-discovery/src/etcd.rs` | etcd 客户端 | ✅ |
| `uhorse-discovery/src/consul.rs` | Consul 客户端 | ✅ |
| `uhorse-discovery/src/health.rs` | 健康检查 | ✅ |
| `uhorse-discovery/src/registry.rs` | 注册中心抽象 | ✅ |
| `uhorse-discovery/src/types.rs` | 核心类型 | ✅ |

### 1.4 验证方案

```bash
# 注册服务
etcdctl put /uhorse/services/gateway/instance-1 '{"host":"10.0.0.1","port":8080}'

# 发现服务
etcdctl get /uhorse/services/gateway --prefix

# 健康检查
curl http://localhost:8080/health/ready
```

---

## 2. 负载均衡

### 2.1 功能需求

- 4 种策略: 轮询、加权、健康感知、最少连接
- 动态实例列表更新
- 统计信息收集

### 2.2 技术设计

```rust
// LoadBalancer trait
#[async_trait]
pub trait LoadBalancer: Send + Sync {
    async fn select(&self, instances: &[ServiceInstance]) -> Option<ServiceInstance>;
    async fn update_stats(&self, instance_id: &str, stats: InstanceStats);
    fn name(&self) -> &str;
}

// 策略枚举
pub enum LoadBalanceStrategy {
    RoundRobin,      // 轮询
    Weighted,        // 加权
    HealthAware,     // 健康感知
    LeastConnection, // 最少连接
}
```

### 2.3 实现文件

| 文件 | 功能 | 状态 |
|------|------|------|
| `uhorse-gateway/src/lb/mod.rs` | 模块定义、工厂 | ✅ |
| `uhorse-gateway/src/lb/round_robin.rs` | 轮询策略 | ✅ |
| `uhorse-gateway/src/lb/weighted.rs` | 加权策略 | ✅ |
| `uhorse-gateway/src/lb/health_aware.rs` | 健康感知策略 | ✅ |
| `uhorse-gateway/src/lb/least_connection.rs` | 最少连接策略 | ✅ |

### 2.4 验证方案

```bash
# 轮询测试
for i in {1..10}; do curl http://localhost:8080/api/v1/agents; done

# 加权测试 (配置不同权重)
# 期望: 高权重实例获得更多流量
```

---

## 3. 分布式配置中心

### 3.1 功能需求

- etcd 存储配置
- 配置热加载 (Watch 机制)
- 版本控制与回滚
- 本地缓存 + 降级

### 3.2 技术设计

```rust
// ConfigBackend trait
#[async_trait]
pub trait ConfigBackend: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<String>>;
    async fn set(&self, key: &str, value: &str) -> Result<()>;
    async fn delete(&self, key: &str) -> Result<()>;
    async fn list(&self, prefix: &str) -> Result<Vec<String>>;
    async fn watch(&self, key: &str) -> Result<ConfigWatchStream>;
}

// 热加载管理器
pub struct HotReloadManager {
    subscribers: HashMap<String, Vec<Arc<dyn ConfigReloader>>>,
}
```

### 3.3 实现文件

| 文件 | 功能 | 状态 |
|------|------|------|
| `uhorse-config/src/distributed.rs` | 分布式配置客户端 | ✅ |
| `uhorse-config/src/hot_reload.rs` | 热加载机制 | ✅ |
| `uhorse-config/src/versioning.rs` | 版本管理 | ✅ |

### 3.4 验证方案

```bash
# 设置配置
etcdctl put /uhorse/config/server.timeout "30"

# 验证热加载
# 修改配置后观察日志输出 "Config reloaded"

# 版本回滚
curl -X POST http://localhost:8080/api/v1/config/rollback?key=server.timeout&version=1
```

---

## 4. 依赖配置

### 4.1 Cargo.toml

```toml
# uhorse-discovery
[dependencies]
etcd-client = { version = "0.13", optional = true }
consul = { version = "0.4", optional = true }

[features]
default = ["etcd"]
etcd = ["dep:etcd-client"]
consul = ["dep:consul"]
full = ["etcd", "consul", "health-check"]
```

### 4.2 环境变量

```bash
# etcd 配置
ETCD_ENDPOINTS=http://localhost:2379
ETCD_USERNAME=
ETCD_PASSWORD=

# Consul 配置 (备选)
CONSUL_ADDR=http://localhost:8500
CONSUL_TOKEN=
```

---

## 5. 里程碑验收

### 5.1 功能验收

- [x] etcd 服务发现正常
- [x] 4 种负载均衡策略工作正常
- [x] 分布式配置读写正常
- [x] 配置热加载生效
- [x] 版本回滚功能正常

### 5.2 性能验收

| 指标 | 目标 | 实际 |
|------|------|------|
| 服务发现延迟 | < 10ms | ✅ |
| 负载均衡决策 | < 1ms | ✅ |
| 配置读取延迟 | < 5ms | ✅ |
| 故障转移时间 | < 30s | 待测试 |

### 5.3 集群测试

```bash
# 启动 3 节点集群
docker-compose -f deployments/ha/docker-compose.yml up -d

# 故障转移测试
docker kill uhorse-node-1
# 验证: 服务在 30s 内恢复
curl http://localhost:8080/health/ready
```

---

## 6. 后续优化

- [ ] 添加 DNS 服务发现支持
- [ ] 实现服务网格集成 (可选)
- [ ] 添加 Prometheus 指标导出
- [ ] 完善故障转移测试

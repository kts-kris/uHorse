# Phase 2: 可扩展性架构

> 说明：本文档是 **路线图 / 阶段规划材料**，用于记录当时的目标、设计和阶段状态，不应直接视为当前仓库主线已实现能力清单。当前主线请以 `docs/architecture/v4.0-architecture.md`、`README.md`、`LOCAL_SETUP.md` 为准。

## 概述

**目标**: 实现数据库分片、分布式缓存、消息队列，支持 10 节点集群，100K 并发用户

**周期**: 5 周

**状态**: 📋 计划中

---

## 1. 数据库分片

### 1.1 分片策略

```rust
pub enum ShardingStrategy {
    // 按 tenant_id 分片
    TenantBased,
    // 按 user_id 哈希分片
    HashBased,
    // 按时间范围分片
    RangeBased,
}
```

### 1.2 实现清单

| 文件 | 功能 | 状态 |
|------|------|------|
| `uhorse-storage/src/sharding/strategy.rs` | 分片策略定义 | 📋 |
| `uhorse-storage/src/sharding/router.rs` | 请求路由 | 📋 |
| `uhorse-storage/src/sharding/replica.rs` | 读写分离 | 📋 |
| `uhorse-storage/src/sharding/migration.rs` | 数据迁移 | 📋 |

### 1.3 配置示例

```toml
[sharding]
strategy = "tenant_based"
shards = [
    { id = 1, dsn = "sqlite://data/shard1.db" },
    { id = 2, dsn = "sqlite://data/shard2.db" },
]
replicas = [
    { shard_id = 1, dsn = "sqlite://data/shard1_replica.db" },
]
```

---

## 2. 分布式缓存 (Redis)

### 2.1 功能需求

- Redis 集成
- 会话缓存
- 令牌黑名单
- 缓存策略 (LRU/LFU/TTL)

### 2.2 实现清单

| 文件 | 功能 | 状态 |
|------|------|------|
| `uhorse-cache/src/redis.rs` | Redis 客户端 | 📋 |
| `uhorse-cache/src/session.rs` | 会话缓存 | 📋 |
| `uhorse-cache/src/token_blacklist.rs` | 令牌黑名单 | 📋 |
| `uhorse-cache/src/policy.rs` | 缓存策略 | 📋 |

### 2.3 配置示例

```toml
[cache.redis]
url = "redis://localhost:6379"
pool_size = 10
ttl = 3600

[cache.session]
prefix = "session:"
ttl = 86400
```

---

## 3. 消息队列 (NATS)

### 3.1 功能需求

- NATS JetStream 集成
- 任务队列
- 死信队列
- 重试策略

### 3.2 实现清单

| 文件 | 功能 | 状态 |
|------|------|------|
| `uhorse-queue/src/nats.rs` | NATS 客户端 | 📋 |
| `uhorse-queue/src/task_queue.rs` | 任务队列 | 📋 |
| `uhorse-queue/src/dead_letter.rs` | 死信队列 | 📋 |
| `uhorse-queue/src/retry.rs` | 重试策略 | 📋 |

### 3.3 配置示例

```toml
[queue.nats]
url = "nats://localhost:4222"
stream = "uhorse-tasks"

[queue.retry]
max_attempts = 3
backoff = "exponential"
initial_delay_ms = 1000
```

---

## 4. 里程碑验收

### 4.1 功能验收

- [ ] 分片读写正常
- [ ] Redis 缓存命中率 > 90%
- [ ] NATS 消息投递可靠

### 4.2 性能验收

| 指标 | 目标 |
|------|------|
| 分片写入吞吐 | > 10K TPS |
| 缓存命中延迟 | < 1ms |
| 消息队列延迟 | < 10ms |
| 并发用户 | 100K+ |

### 4.3 测试命令

```bash
# 分片测试
curl -X POST http://localhost:8080/api/v1/agents \
  -H "X-Tenant-ID: tenant-001" \
  -d '{"name": "test"}'

# Redis 测试
redis-cli GET "session:tenant-001:user-123"

# NATS 测试
nats sub uhorse.tasks --count 10
```

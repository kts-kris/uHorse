# Phase 4: 数据治理体系

## 概述

**目标**: 实现数据分类、备份恢复、灾难恢复，确保 RTO < 4h, RPO < 1h

**周期**: 3 周

**状态**: 📋 计划中

---

## 1. 数据分类框架

### 1.1 分类级别

```rust
pub enum DataClassification {
    // 公开 - 可自由分享
    Public,
    // 内部 - 仅限内部使用
    Internal,
    // 机密 - 需要授权访问
    Confidential,
    // 高度机密 - 严格限制
    Restricted,
}

pub struct DataClassificationRule {
    pub table: String,
    pub column: Option<String>,
    pub classification: DataClassification,
    pub retention_days: Option<u32>,
    pub requires_encryption: bool,
    pub access_log: bool,
}
```

### 1.2 实现清单

| 文件 | 功能 | 状态 |
|------|------|------|
| `uhorse-governance/src/lib.rs` | 模块定义 | 📋 |
| `uhorse-governance/src/classification.rs` | 分类框架 | 📋 |
| `uhorse-governance/src/retention.rs` | 保留策略 | 📋 |
| `uhorse-governance/src/archive.rs` | 归档机制 | 📋 |

### 1.3 默认分类规则

| 表 | 字段 | 分类 | 保留期 |
|----|------|------|--------|
| users | email | Confidential | 永久 |
| users | phone | Confidential | 永久 |
| sessions | * | Confidential | 90 天 |
| messages | content | Restricted | 365 天 |
| audit_logs | * | Confidential | 2555 天 (7 年) |
| metrics | * | Internal | 30 天 |

---

## 2. 数据保留策略

### 2.1 保留规则

```rust
pub struct RetentionPolicy {
    pub name: String,
    pub classification: DataClassification,
    pub retention_days: u32,
    pub action_after_expiry: ExpiryAction,
    pub archive_before_delete: bool,
}

pub enum ExpiryAction {
    // 自动删除
    Delete,
    // 归档
    Archive,
    // 标记待审核
    FlagForReview,
    // 不操作
    None,
}
```

### 2.2 配置示例

```toml
[retention.policies]
sessions = { retention_days = 90, action = "delete" }
messages = { retention_days = 365, action = "archive", archive_days = 2555 }
audit_logs = { retention_days = 2555, action = "none" }
metrics = { retention_days = 30, action = "delete" }

[retention.scheduler]
enabled = true
run_hour = 3  # 凌晨 3 点执行
batch_size = 10000
```

### 2.3 生命周期流程

```
数据创建 → 分类标记 → 活跃使用 → 过期检查 → 归档/删除
                                    ↓
                              保留期审核
```

---

## 3. 备份系统

### 3.1 备份策略

```rust
pub enum BackupType {
    // 完整备份
    Full,
    // 增量备份
    Incremental,
    // 差异备份
    Differential,
}

pub struct BackupSchedule {
    pub backup_type: BackupType,
    pub cron: String,
    pub retention_count: u32,
    pub encryption: bool,
    pub compression: CompressionType,
}
```

### 3.2 实现清单

| 文件 | 功能 | 状态 |
|------|------|------|
| `uhorse-backup/src/lib.rs` | 模块定义 | 📋 |
| `uhorse-backup/src/scheduler.rs` | 备份调度 | 📋 |
| `uhorse-backup/src/full.rs` | 完整备份 | 📋 |
| `uhorse-backup/src/incremental.rs` | 增量备份 | 📋 |
| `uhorse-backup/src/encryption.rs` | 备份加密 | 📋 |
| `uhorse-backup/src/compression.rs` | 压缩存储 | 📋 |
| `uhorse-backup/src/restore.rs` | 恢复工具 | 📋 |

### 3.3 配置示例

```toml
[backup]
enabled = true
storage_path = "/var/lib/uhorse/backups"
encryption_key_id = "backup-key-001"

[backup.schedule.full]
cron = "0 2 * * 0"  # 每周日凌晨 2 点
retention_count = 4  # 保留 4 个

[backup.schedule.incremental]
cron = "0 2 * * 1-6"  # 周一到周六凌晨 2 点
retention_count = 14  # 保留 14 个

[backup.compression]
algorithm = "zstd"  # gzip | zstd | xz
level = 3

[backup.storage]
# 本地存储
type = "local"
path = "/var/lib/uhorse/backups"

# S3 存储 (可选)
# type = "s3"
# bucket = "uhorse-backups"
# region = "us-east-1"
# prefix = "production/"
```

### 3.4 备份内容

```
backups/
├── full/
│   ├── 2024-01-01_020000/
│   │   ├── metadata.json
│   │   ├── database.db.gz.enc
│   │   └── checksum.sha256
│   └── ...
├── incremental/
│   ├── 2024-01-02_020000/
│   │   ├── metadata.json
│   │   ├── wal-001.gz.enc
│   │   └── checksum.sha256
│   └── ...
└── latest -> full/2024-01-01_020000/
```

---

## 4. 恢复系统

### 4.1 恢复类型

```rust
pub enum RecoveryType {
    // 完整恢复
    Full,
    // 时间点恢复 (PITR)
    PointInTime(DateTime<Utc>),
    // 表级恢复
    Table(String),
    // 行级恢复
    Row { table: String, id: String },
}

pub struct RecoveryOptions {
    pub recovery_type: RecoveryType,
    pub backup_id: Option<String>,
    pub target_time: Option<DateTime<Utc>>,
    pub dry_run: bool,
    pub verify_only: bool,
}
```

### 4.2 恢复 API

```rust
// POST /api/v1/backup/restore
pub async fn restore_backup(
    options: RecoveryOptions,
) -> Result<RecoveryResult>;

// GET /api/v1/backup/list
pub async fn list_backups(
    backup_type: Option<BackupType>,
    limit: Option<u32>,
) -> Result<Vec<BackupInfo>>;

// GET /api/v1/backup/:id/verify
pub async fn verify_backup(
    backup_id: &str,
) -> Result<VerificationResult>;
```

### 4.3 恢复流程

```
1. 选择备份点
   ↓
2. 完整性校验 (checksum)
   ↓
3. 解密 + 解压
   ↓
4. 停止服务 (可选)
   ↓
5. 恢复数据
   ↓
6. 应用增量 (如需要)
   ↓
7. 一致性检查
   ↓
8. 重启服务
```

---

## 5. 灾难恢复 (DR)

### 5.1 DR 架构

```
┌─────────────────┐     ┌─────────────────┐
│   主站点 (AZ-1)  │────▶│   备站点 (AZ-2)  │
│                 │     │                 │
│  ┌───────────┐  │     │  ┌───────────┐  │
│  │ uHorse    │  │     │  │ uHorse    │  │
│  │ Primary   │──┼────▶│  │ Standby   │  │
│  └───────────┘  │     │  └───────────┘  │
│                 │     │                 │
│  ┌───────────┐  │     │  ┌───────────┐  │
│  │ Database  │──┼────▶│  │ Database  │  │
│  │ Primary   │  │     │  │ Replica   │  │
│  └───────────┘  │     │  └───────────┘  │
└─────────────────┘     └─────────────────┘
```

### 5.2 DR 计划

| 场景 | RTO | RPO | 动作 |
|------|-----|-----|------|
| 单节点故障 | < 30s | 0 | 自动故障转移 |
| AZ 故障 | < 1h | < 1min | DNS 切换 |
| 区域故障 | < 4h | < 1h | 跨区域恢复 |
| 数据损坏 | < 4h | < 1h | 备份恢复 |

### 5.3 实现清单

| 文件 | 功能 | 状态 |
|------|------|------|
| `deployments/dr/plan.md` | DR 计划文档 | 📋 |
| `deployments/dr/failover.rs` | 故障转移脚本 | 📋 |
| `deployments/dr/drill.yml` | 演练计划 | 📋 |
| `uhorse-backup/src/replication.rs` | 跨区域复制 | 📋 |

### 5.4 故障转移检查清单

```markdown
## 故障转移前检查

- [ ] 确认主站点不可用
- [ ] 检查备站点健康状态
- [ ] 验证数据同步状态
- [ ] 通知相关团队
- [ ] 准备回滚方案

## 故障转移执行

- [ ] 更新 DNS 记录
- [ ] 激活备站点
- [ ] 验证服务可用性
- [ ] 监控错误率
- [ ] 通知用户

## 故障转移后

- [ ] 监控系统稳定性
- [ ] 记录事件详情
- [ ] 制定主站点恢复计划
```

---

## 6. 数据归档

### 6.1 归档策略

```rust
pub struct ArchivePolicy {
    // 源表
    pub source_table: String,
    // 归档条件
    pub condition: String,  // SQL WHERE clause
    // 目标存储
    pub storage: ArchiveStorage,
    // 归档后操作
    pub post_action: PostArchiveAction,
}

pub enum ArchiveStorage {
    // 冷存储数据库
    ColdStorage(String),
    // 对象存储
    ObjectStorage { bucket: String, prefix: String },
    // 文件系统
    FileSystem(String),
}

pub enum PostArchiveAction {
    // 保留原数据
    Keep,
    // 删除原数据
    Delete,
    // 标记为已归档
    MarkArchived,
}
```

### 6.2 配置示例

```toml
[archive.policies.messages]
condition = "created_at < datetime('now', '-365 days')"
storage = { type = "cold_storage", path = "/archive/messages" }
post_action = "delete"
schedule = "0 4 * * *"  # 每天凌晨 4 点

[archive.policies.audit_logs]
condition = "created_at < datetime('now', '-2555 days')"
storage = { type = "object_storage", bucket = "uhorse-archive", prefix = "audit/" }
post_action = "keep"
schedule = "0 4 * * 0"  # 每周日凌晨 4 点
```

---

## 7. 里程碑验收

### 7.1 功能验收

- [ ] 数据分类标记生效
- [ ] 保留策略自动执行
- [ ] 完整备份成功
- [ ] 增量备份成功
- [ ] 备份加密正确
- [ ] 恢复功能正常
- [ ] 跨区域复制工作
- [ ] 归档流程正常

### 7.2 性能验收

| 指标 | 目标 |
|------|------|
| RTO (恢复时间) | < 4h |
| RPO (数据丢失) | < 1h |
| 备份压缩率 | > 50% |
| 增量备份时间 | < 30min |
| 恢复验证时间 | < 15min |

### 7.3 测试命令

```bash
# 创建备份
curl -X POST http://localhost:8080/api/v1/backup \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -d '{"type": "full"}'

# 列出备份
curl http://localhost:8080/api/v1/backup/list

# 验证备份
curl http://localhost:8080/api/v1/backup/backup-001/verify

# 恢复测试 (dry-run)
curl -X POST http://localhost:8080/api/v1/backup/restore \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -d '{"backup_id": "backup-001", "dry_run": true}'

# 数据分类查询
sqlite3 data/uhorse.db "SELECT * FROM data_classifications"

# 保留策略执行日志
grep "retention" /var/log/uhorse/governance.log
```

---

## 8. 运维手册

### 8.1 日常运维

```bash
# 检查备份状态
./scripts/backup-status.sh

# 手动触发备份
./scripts/backup-now.sh --type full

# 验证最近备份
./scripts/verify-latest-backup.sh

# 检查数据分类覆盖率
./scripts/check-classification.sh
```

### 8.2 定期演练

| 演练类型 | 频率 | 执行时间 |
|----------|------|----------|
| 备份恢复测试 | 每周 | 周六 10:00 |
| DR 切换演练 | 每季度 | 季末周末 |
| 完整灾难恢复 | 每年 | 年初 |

### 8.3 监控告警

```yaml
# 备份告警规则
- alert: BackupFailed
  expr: backup_status == 0
  for: 5m
  annotations:
    summary: "备份任务失败"

- alert: BackupTooOld
  expr: time() - backup_last_success > 86400
  annotations:
    summary: "备份超过 24 小时未执行"

- alert: StorageSpaceLow
  expr: backup_storage_free_percent < 20
  annotations:
    summary: "备份存储空间不足 20%"
```

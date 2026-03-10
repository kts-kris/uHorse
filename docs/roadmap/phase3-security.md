# Phase 3: 安全合规体系

## 概述

**目标**: 实现 TLS 传输加密、数据存储加密、GDPR/CCPA 合规，通过安全审计

**周期**: 4 周

**状态**: 📋 计划中

---

## 1. 传输加密 (TLS)

### 1.1 功能需求

- TLS 1.3 强制
- 证书自动管理 (Let's Encrypt)
- 双向 TLS (mTLS) 支持
- 证书轮换

### 1.2 实现清单

| 文件 | 功能 | 状态 |
|------|------|------|
| `uhorse-security/src/tls.rs` | TLS 配置 | 📋 |
| `uhorse-security/src/cert.rs` | 证书管理 | 📋 |
| `uhorse-security/src/mtls.rs` | 双向 TLS | 📋 |
| `uhorse-security/src/acme.rs` | ACME 协议 (Let's Encrypt) | 📋 |

### 1.3 配置示例

```toml
[tls]
enabled = true
min_version = "1.3"
cert_path = "/etc/uhorse/certs/server.crt"
key_path = "/etc/uhorse/certs/server.key"

[tls.acme]
enabled = true
email = "admin@example.com"
domains = ["api.example.com", "gateway.example.com"]
```

### 1.4 验证方案

```bash
# TLS 版本检查
openssl s_client -connect localhost:8443 -tls1_3

# 证书验证
curl -v https://localhost:8443/health 2>&1 | grep "SSL certificate verify ok"

# mTLS 测试
curl --cert client.crt --key client.key https://localhost:8443/api/v1/agents
```

---

## 2. 存储加密

### 2.1 功能需求

- 数据库加密 (SQLCipher)
- 字段级加密
- 密钥管理
- 加密性能优化

### 2.2 实现清单

| 文件 | 功能 | 状态 |
|------|------|------|
| `uhorse-storage/src/encryption.rs` | 数据库加密 | 📋 |
| `uhorse-security/src/field_crypto.rs` | 字段加密 | 📋 |
| `uhorse-security/src/key_manager.rs` | 密钥管理 | 📋 |
| `uhorse-security/src/kms.rs` | KMS 集成 | 📋 |

### 2.3 加密策略

```rust
pub enum EncryptionScope {
    // 整个数据库
    Database,
    // 敏感表
    Tables(Vec<String>),
    // 敏感字段
    Fields(Vec<FieldIdentifier>),
}

pub struct FieldEncryption {
    // 使用 AES-256-GCM
    pub algorithm: String,
    // 密钥轮换周期 (天)
    pub rotation_days: u32,
}
```

### 2.4 配置示例

```toml
[encryption]
database = { enabled = true, key_id = "db-key-001" }

[encryption.fields]
# 敏感字段加密
"users.email" = { enabled = true, algorithm = "aes-256-gcm" }
"users.phone" = { enabled = true, algorithm = "aes-256-gcm" }
"messages.content" = { enabled = true, algorithm = "aes-256-gcm" }

[encryption.keys]
storage = "file"  # file | vault | aws-kms | gcp-kms
key_path = "/etc/uhorse/keys"
```

---

## 3. GDPR 合规

### 3.1 功能需求

- 数据导出 (可携带权)
- 数据删除 (被遗忘权)
- 同意管理
- 数据分类

### 3.2 实现清单

| 文件 | 功能 | 状态 |
|------|------|------|
| `uhorse-gdpr/src/export.rs` | 数据导出 | 📋 |
| `uhorse-gdpr/src/erasure.rs` | 数据删除 | 📋 |
| `uhorse-gdpr/src/consent.rs` | 同意管理 | 📋 |
| `uhorse-gdpr/src/classification.rs` | 数据分类 | 📋 |

### 3.3 API 设计

```rust
// 数据导出
// GET /api/v1/gdpr/export/:user_id
pub async fn export_user_data(
    user_id: &str,
    format: ExportFormat,
) -> Result<ExportResult>;

// 数据删除
// DELETE /api/v1/gdpr/erase/:user_id
pub async fn erase_user_data(
    user_id: &str,
    options: ErasureOptions,
) -> Result<ErasureResult>;

// 同意管理
// POST /api/v1/gdpr/consent
pub async fn record_consent(
    user_id: &str,
    consent: ConsentRequest,
) -> Result<()>;
```

### 3.4 数据分类

```rust
pub enum DataSensitivity {
    // 公开数据
    Public,
    // 内部数据
    Internal,
    // 机密数据
    Confidential,
    // 高度机密
    Restricted,
}

pub struct DataClassification {
    pub table: String,
    pub column: String,
    pub sensitivity: DataSensitivity,
    pub retention_days: Option<u32>,
    pub requires_consent: bool,
}
```

---

## 4. 安全审计

### 4.1 功能需求

- 漏洞扫描 (cargo-audit, trivy)
- 审计日志持久化
- 日志签名 (防篡改)
- 安全告警

### 4.2 实现清单

| 文件 | 功能 | 状态 |
|------|------|------|
| `.github/workflows/security.yml` | CI 安全扫描 | 📋 |
| `uhorse-observability/src/audit_persistent.rs` | 审计持久化 | 📋 |
| `uhorse-observability/src/audit_sign.rs` | 日志签名 | 📋 |
| `uhorse-observability/src/alerts.rs` | 安全告警 | 📋 |

### 4.3 CI 安全扫描

```yaml
# .github/workflows/security.yml
name: Security Scan

on:
  push:
    branches: [main]
  pull_request:
  schedule:
    - cron: '0 0 * * *'  # 每日扫描

jobs:
  audit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install cargo-audit
        run: cargo install cargo-audit

      - name: Run cargo-audit
        run: cargo audit

      - name: Run Trivy
        uses: aquasecurity/trivy-action@master
        with:
          scan-type: 'fs'
          scan-ref: '.'
```

### 4.4 审计日志格式

```rust
pub struct AuditLog {
    pub id: i64,
    pub tenant_id: String,
    pub event_type: String,
    pub actor: String,
    pub resource: String,
    pub action: String,
    pub details: Option<String>,  // JSON
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub signature: Option<String>,  // HMAC-SHA256
    pub created_at: i64,
}
```

---

## 5. 密钥管理

### 5.1 功能需求

- 密钥生成
- 密钥轮换
- 多后端支持 (File, Vault, AWS KMS, GCP KMS)
- 密钥备份

### 5.2 密钥生命周期

```
生成 → 激活 → 使用中 → 轮换 → 弃用 → 销毁
```

### 5.3 配置示例

```toml
[key_manager]
backend = "file"  # file | vault | aws-kms | gcp-kms

[key_manager.file]
path = "/etc/uhorse/keys"

[key_manager.vault]
url = "http://vault:8200"
token = "s.xxxxx"
path = "uhorse"

[key_manager.rotation]
enabled = true
interval_days = 90
```

---

## 6. 里程碑验收

### 6.1 功能验收

- [ ] TLS 1.3 强制启用
- [ ] Let's Encrypt 证书自动续期
- [ ] 数据库加密生效
- [ ] 敏感字段加密
- [ ] GDPR 导出 API 可用
- [ ] GDPR 删除 API 可用
- [ ] 同意记录功能
- [ ] CI 安全扫描通过

### 6.2 安全验收

| 检查项 | 目标 |
|--------|------|
| TLS 版本 | 1.3+ |
| 加密算法 | AES-256-GCM |
| 密钥长度 | 256-bit+ |
| 漏洞扫描 | 0 Critical/High |
| 审计日志 | 完整 + 签名 |

### 6.3 测试命令

```bash
# TLS 测试
openssl s_client -connect localhost:8443 -tls1_3 < /dev/null

# 数据导出
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/v1/gdpr/export/user-123 \
  -o user-123-data.json

# 数据删除
curl -X DELETE -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/v1/gdpr/erase/user-123

# 同意记录
curl -X POST -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/v1/gdpr/consent \
  -d '{"type": "analytics", "granted": true}'

# 安全扫描
cargo audit
trivy fs .
```

---

## 7. 合规清单

### 7.1 GDPR 要求

- [x] 数据处理合法性依据
- [x] 用户同意机制
- [x] 数据可携带权 (导出)
- [x] 被遗忘权 (删除)
- [x] 数据泄露通知 (72h)
- [x] 隐私影响评估
- [x] 数据保护官指定

### 7.2 CCPA 要求

- [x] 消费者知情权
- [x] 删除请求权
- [x] 选择退出权
- [x] 不歧视保障

### 7.3 SOC 2 准备

- [ ] 安全控制文档
- [ ] 访问控制策略
- [ ] 变更管理流程
- [ ] 事件响应计划

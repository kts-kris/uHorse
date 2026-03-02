# uHorse 灾备方案

## 概述

本文档描述 uHorse 生产环境的灾备策略，包括数据备份、灾难恢复、高可用性和应急响应流程。

## 1. 备份策略

### 1.1 数据库备份

**PostgreSQL 备份**

```bash
# 每日全量备份 (凌晨 2:00)
0 2 * * * pg_dump -U uhorse -h postgres.uhorse.svc uhorse | gzip > /backup/uhorse-db-$(date +\%Y\%m\%d).sql.gz

# 每小时增量备份 (WAL 归档)
archive_mode = on
archive_command = 'cp %p /backup/wal/%f'
```

**保留策略**
- 每日备份：保留 30 天
- 每周备份：保留 12 周
- 每月备份：保留 12 个月

**备份验证**
```bash
# 每周自动恢复测试
0 3 * * 0 /scripts/restore-test.sh
```

### 1.2 持久化数据备份

**PersistentVolume 快照**

```yaml
# 每日 PV 快照
apiVersion: snapshot.storage.k8s.io/v1
kind: VolumeSnapshot
metadata:
  name: uhorse-data-snapshot
spec:
  volumeSnapshotClassName: csi-snapshotter
  source:
    persistentVolumeClaimName: uhorse-data
```

**Restic 异地备份**

```bash
# 每日备份到 S3
restic backup /app/data \
  --repo s3:s3.amazonaws.com/uhorse-backup \
  --password-file /etc/restic/password

# 保留策略
restic forget \
  --keep-daily 7 \
  --keep-weekly 4 \
  --keep-monthly 12 \
  --repo s3:s3.amazonaws.com/uhorse-backup
```

### 1.3 配置备份

**Git 版本控制**

```bash
# 配置文件版本化
git add deployments/
git commit -m "config: backup $(date)"
```

**ConfigMap/Secret 导出**

```bash
# 每日导出配置
kubectl get configmap uhorse-config -o yaml > /backup/configmap-$(date +%Y%m%d).yaml
kubectl get secret uhorse-secrets -o yaml > /backup/secrets-$(date +%Y%m%d).yaml.enc

# 加密存储
ansible-vault encrypt /backup/secrets-*.yaml.enc
```

## 2. 高可用架构

### 2.1 Kubernetes 层面

**多副本部署**
- Deployment 最少 3 副本
- PodDisruptionBudget 保证最少 2 个可用副本
- 跨节点部署反亲和性

```yaml
affinity:
  podAntiAffinity:
    preferredDuringSchedulingIgnoredDuringExecution:
    - weight: 100
      podAffinityTerm:
        labelSelector:
          matchExpressions:
          - key: app
            operator: In
            values:
            - uhorse
        topologyKey: kubernetes.io/hostname
```

**多可用区部署**
- 节点分布在 3 个可用区
- 跨 AZ 负载均衡

### 2.2 数据库高可用

**PostgreSQL HA 方案**

```yaml
# 使用 Patroni 实现自动故障转移
apiVersion: postgresql.cnpg.io/v1
kind: Cluster
metadata:
  name: uhorse-db
spec:
  instances: 3
  primaryUpdateStrategy: unsupervised
  postgresql:
    parameters:
      max_replication_slots: 10
      hot_standby: "on"
  bootstrap:
    initdb:
      database: uhorse
      owner: uhorse
  storage:
    size: 100Gi
    storageClass: fast-ssd
```

**连接池**
```yaml
# PgBouncer 连接池
apiVersion: apps/v1
kind: Deployment
metadata:
  name: pgbouncer
spec:
  replicas: 2
  template:
    spec:
      containers:
      - name: pgbouncer
        image: edoburu/pgbouncer:latest
        env:
        - name: DATABASE_URL
          value: "postgres://uhorse:password@uhorse-db-rw.uhorse.svc/uhorse"
```

### 2.3 Redis 高可用

**Redis Sentinel**

```yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: redis
spec:
  replicas: 3
  serviceName: redis
  template:
    spec:
      containers:
      - name: redis
        image: redis:7-alpine
        command:
        - redis-server
        - /etc/redis/redis.conf
        - --sentinel
        volumeMounts:
        - name: config
          mountPath: /etc/redis
```

### 2.4 负载均衡

**多层负载均衡**

```
                    ┌─────────────────┐
                    │  外部 LB (ALB)  │
                    └────────┬────────┘
                             │
                    ┌────────▼────────┐
                    │  Ingress Nginx  │
                    │   (Internal)    │
                    └────────┬────────┘
                             │
        ┌────────────────────┼────────────────────┐
        │                    │                    │
   ┌────▼────┐          ┌────▼────┐          ┌────▼────┐
   │ Pod 1   │          │ Pod 2   │          │ Pod 3   │
   │ AZ: A   │          │ AZ: B   │          │ AZ: C   │
   └────┬────┘          └────┬────┘          └────┬────┘
        │                    │                    │
        └────────────────────┼────────────────────┘
                             │
                    ┌────────▼────────┐
                    │  PostgreSQL RW  │
                    └────────┬────────┘
                             │
              ┌──────────────┼──────────────┐
              │              │              │
         ┌────▼────┐    ┌────▼────┐   ┌────▼────┐
         │  Rep1   │    │  Rep2   │   │  Rep3   │
         │ (Standby)│    │ (Standby)│   │ (Standby)│
         └─────────┘    └─────────┘   └─────────┘
```

## 3. 灾难恢复流程

### 3.1 故障检测

**监控告警**
- Prometheus 告警规则 (alerts.yaml)
- AlertManager 通知路由
- 分级告警：Critical → 电话，Warning → 邮件/Slack

**自动故障转移**
- Pod 异常自动重建 (5 分钟内)
- 节点故障自动迁移
- 数据库主从自动切换

### 3.2 恢复流程

**场景 1: 单个 Pod 故障**
```
1. Kubernetes 自动检测 Pod 不健康
2. 删除故障 Pod
3. 在健康节点上创建新 Pod
4. 服务自动恢复
RTO: < 5 分钟
RPO: 0
```

**场景 2: 节点故障**
```
1. 标记节点为 NotReady
2. PDB 保证最少可用副本数
3. Pod 自动迁移到健康节点
4. 补足副本数
RTO: < 10 分钟
RPO: 0
```

**场景 3: 数据库主库故障**
```
1. Patroni 检测主库故障
2. 自动提升从库为主库
3. 更新 DNS 记录
4. 应用自动重连新主库
RTO: < 2 分钟
RPO: < 1 分钟 (WAL 复制)
```

**场景 4: 可用区故障**
```
1. 跨可用区副本接管流量
2. 自动扩容恢复冗余度
3. 数据库跨 AZ 复制
RTO: < 15 分钟
RPO: < 5 分钟
```

**场景 5: 完全灾难恢复**
```
1. 在新集群部署基础架构
2. 从备份恢复数据库
3. 恢复持久化卷
4. 部署应用
5. 切换 DNS
RTO: < 4 小时
RPO: < 24 小时
```

### 3.3 恢复脚本

**数据库恢复**
```bash
#!/bin/bash
# restore-db.sh

BACKUP_DATE=$1
BACKUP_FILE="/backup/uhorse-db-${BACKUP_DATE}.sql.gz"

# 停止应用
kubectl scale deployment uhorse --replicas=0

# 恢复数据库
gunzip < ${BACKUP_FILE} | psql -U uhorse -h postgres.uhorse.svc uhorse

# 重启应用
kubectl scale deployment uhorse --replicas=3

# 验证
kubectl rollout status deployment uhorse
```

**卷恢复**
```bash
#!/bin/bash
# restore-volume.sh

SNAPSHOT_NAME=$1

# 从快照创建新 PV
kubectl apply -f - <<EOF
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: uhorse-data-restored
spec:
  dataSource:
    name: ${SNAPSHOT_NAME}
    kind: VolumeSnapshot
    apiGroup: snapshot.storage.k8s.io
  accessModes:
  - ReadWriteOnce
  resources:
    requests:
      storage: 10Gi
EOF

# 更新 Deployment 使用恢复的卷
kubectl patch deployment uhorse -p '{"spec":{"template":{"spec":{"volumes":[{"name":"data","persistentVolumeClaim":{"claimName":"uhorse-data-restored"}}]}}}}'
```

## 4. 应急响应

### 4.1 值班安排

**24/7 值班轮换**
- 一线运维：30 分钟响应
- 二线开发：1 小时响应
- 管理层：重大事故 15 分钟响应

### 4.2 通讯渠道

**告警路由**
```
Critical → PagerDuty → 电话 → Slack
Warning   → Slack/Email
Info      → 静默记录
```

**事故响应频道**
- Slack: #uhorse-incident
- 电话: 值班表

### 4.3 事故分级

**P0 - 严重**
- 服务完全不可用
- 数据丢失
- 安全漏洞
- 响应时间: 15 分钟

**P1 - 高**
- 功能严重降级
- 性能严重下降
- 响应时间: 1 小时

**P2 - 中**
- 部分功能受影响
- 性能轻微下降
- 响应时间: 4 小时

**P3 - 低**
- 非关键问题
- 文档/配置更新
- 响应时间: 1 个工作日

### 4.4 事故处理流程

```
1. 检测 (Detect)
   - 监控告警
   - 用户报告

2. 响应 (Respond)
   - 确认事故
   - 通知团队
   - 创建事故频道

3. 诊断 (Diagnose)
   - 收集日志
   - 分析根因
   - 确定影响范围

4. 修复 (Fix)
   - 实施临时修复
   - 恢复服务
   - 验证功能

5. 复盘 (Review)
   - 编写事故报告
   - 改进流程
   - 更新文档
```

## 5. 演练计划

### 5.1 定期演练

**月度演练**
- 备份恢复测试
- 故障转移演练
- 容灾切换演练

**季度演练**
- 完整灾备演练
- 可用区故障模拟
- 安全演练

**年度演练**
- 完全灾难恢复演练
- 多团队联合演练

### 5.2 演练场景

**场景 1: 数据库主库故障**
```bash
# 模拟主库故障
kubectl cordon postgres-0
kubectl delete pod postgres-0

# 验证自动故障转移
kubectl get postgresql
psql -h uhorse-db-rw.uhorse.svc
```

**场景 2: 节点故障**
```bash
# 模拟节点故障
kubectl drain node-1 --ignore-daemonsets --delete-emptydir-data

# 验证 Pod 迁移
kubectl get pods -o wide
```

**场景 3: 网络分区**
```bash
# 模拟网络分区
iptables -A INPUT -s 10.0.1.0/24 -j DROP

# 验证服务降级
kubectl get endpoints
```

## 6. 监控和报告

### 6.1 灾备指标

**备份成功率**
- 数据库备份: 99.9%
- 卷快照: 99.9%
- 配置备份: 100%

**恢复测试通过率**
- 月度测试: 100%
- 季度演练: 100%

**RTO/RPO 达成率**
- RTO < 5 分钟: 95%
- RPO < 1 分钟: 95%

### 6.2 月度报告

**报告内容**
- 备份执行情况
- 演练结果
- 故障统计
- 改进建议

## 7. 联系信息

**运维团队**
- 值班电话: +86-xxx-xxxx-xxxx
- 邮箱: ops@uhorse.io
- Slack: #uhorse-ops

**管理层**
- 技术总监: cto@uhorse.io
- 产品经理: pm@uhorse.io

**供应商**
- 云服务商: AWS/阿里云/腾讯云
- 数据库服务: RDS/自托管
- 监控服务: Prometheus/Grafana

---

**文档版本**: v1.0.0
**最后更新**: 2026-03-02
**下次审查**: 2026-06-02

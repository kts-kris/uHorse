# IT Support Template

IT 技术支持模板，支持故障排查、工单管理、知识库查询。

## 功能特性

- 🔧 故障诊断与排查
- 🎫 IT 工单创建与跟踪
- 📚 技术知识库搜索
- 💻 常见问题自助解决
- 🔄 资产信息查询

## 快速开始

```bash
uhorse init --template it-support
uhorse start
```

## 技能列表

| 技能 | 描述 |
|------|------|
| `troubleshoot` | 故障排查引导 |
| `create_ticket` | 创建 IT 工单 |
| `kb_search` | 知识库搜索 |
| `asset_info` | 查询资产信息 |
| `password_reset` | 密码重置引导 |
| `software_request` | 软件申请 |

## 故障排查流程

```
1. 问题分类（网络/硬件/软件/账号）
2. 收集环境信息
3. 提供自助解决方案
4. 无法解决则创建工单
5. 分配给对应技术团队
```

## 工单优先级

| 级别 | 响应时间 | 适用场景 |
|------|----------|----------|
| P1 紧急 | 15 分钟 | 系统宕机、数据丢失 |
| P2 高 | 1 小时 | 业务中断、多人受影响 |
| P3 中 | 4 小时 | 单人问题、有替代方案 |
| P4 低 | 24 小时 | 咨询、优化建议 |

## 集成说明

### ServiceNow 集成

```toml
[it_support.servicenow]
instance = "your-instance"
username = "${SN_USERNAME}"
password = "${SN_PASSWORD}"
```

### Jira Service Desk

```toml
[it_support.jira]
url = "https://your-company.atlassian.net"
project = "IT"
```

## 最佳实践

1. **知识库维护**: 及时更新常见问题解决方案
2. **快速响应**: 设置合理的 SLA
3. **自助优先**: 引导用户自助解决简单问题
4. **数据驱动**: 分析工单数据优化服务

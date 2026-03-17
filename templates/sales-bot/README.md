# Sales Bot Template

销售助手模板，支持线索筛选、产品推荐、CRM 集成。

## 功能特性

- 🎯 线索资格评估 (BANT)
- 📊 产品智能推荐
- 📅 会议预约安排
- 📝 销售资料分发
- 🔄 CRM 系统集成

## 快速开始

```bash
uhorse init --template sales-bot
uhorse start
```

## 技能列表

| 技能 | 描述 |
|------|------|
| `lead_qualify` | 线索资格评估 |
| `product_recommend` | 产品推荐 |
| `meeting_schedule` | 预约会议 |
| `material_send` | 发送销售资料 |
| `crm_sync` | CRM 数据同步 |
| `quote_generate` | 生成报价单 |

## 线索评估 (BANT)

- **B**udget: 预算情况
- **A**uthority: 决策权限
- **N**eed: 需求紧迫性
- **T**imeline: 采购时间线

## CRM 集成

### Salesforce

```toml
[sales_bot.salesforce]
client_id = "${SF_CLIENT_ID}"
client_secret = "${SF_CLIENT_SECRET}"
instance_url = "https://your-instance.my.salesforce.com"
```

### HubSpot

```toml
[sales_bot.hubspot]
api_key = "${HUBSPOT_API_KEY}"
```

## 销售话术配置

```markdown
# 开场白
您好！我是 [公司名] 的智能销售顾问。了解到您可能对我们的 [产品] 感兴趣...

# 需求挖掘
请问您目前在使用什么方案？遇到了哪些挑战？

# 产品介绍
根据您的需求，我推荐 [产品]，它可以帮您...

# 下一步
方便安排一次演示吗？我可以帮您预约产品专家的时间。
```

## 最佳实践

1. **快速响应**: 5 分钟内响应新线索
2. **个性化**: 根据客户信息定制对话
3. **跟进节奏**: 设置自动跟进提醒
4. **数据沉淀**: 所有互动同步 CRM

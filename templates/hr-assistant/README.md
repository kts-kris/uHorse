# HR Assistant Template

人力资源助手模板，支持员工入职指引、政策查询、假期管理等功能。

## 功能特性

- 👋 新员工入职引导
- 📋 公司政策查询
- 🏖️ 假期余额查询与申请
- 💰 薪资单查询
- 📝 培训课程推荐
- 🎯 绩效评估提醒

## 快速开始

```bash
uhorse init --template hr-assistant
uhorse start
```

## 配置说明

```toml
[hr_assistant]
# HR 系统集成 (workday, sap, bamboo, custom)
hr_system = "none"

# 假期类型配置
leave_types = ["年假", "病假", "事假", "婚假", "产假", "陪产假"]

# 入职引导流程
onboarding_enabled = true
onboarding_days = 5  # 入职引导持续天数
```

## 技能列表

| 技能 | 描述 |
|------|------|
| `policy_search` | 公司政策搜索 |
| `leave_balance` | 查询假期余额 |
| `leave_apply` | 申请假期 |
| `payslip` | 查询薪资单 |
| `training_list` | 培训课程列表 |
| `onboarding` | 入职引导 |

## 目录结构

```
hr-assistant/
├── config.toml
├── data/
│   ├── policies/        # 政策文档
│   └── uhorse.db
├── skills/
│   ├── policy_search/
│   ├── leave_balance/
│   └── payslip/
└── workspace/
    └── default/
        ├── SOUL.md      # HR 助手人设
        └── MEMORY.md
```

## 政策文档格式

```markdown
# 年假政策

## 适用范围
全体正式员工

## 年假天数
- 工龄 1-5 年：5 天
- 工龄 5-10 年：10 天
- 工龄 10 年以上：15 天

## 申请流程
1. 提前 3 天申请
2. 直属领导审批
...
```

## 集成说明

### Workday 集成

```toml
[hr_assistant.workday]
api_url = "https://your.workday.com/api"
tenant = "your_tenant"
client_id = "${WORKDAY_CLIENT_ID}"
client_secret = "${WORKDAY_CLIENT_SECRET}"
```

### 企业微信/钉钉集成

支持在企业微信或钉钉中使用，配置对应渠道即可。

## 最佳实践

1. **政策更新**: 及时更新政策文档
2. **权限控制**: 敏感信息需验证员工身份
3. **隐私保护**: 薪资等信息加密存储
4. **多语言**: 支持国际化公司多语言需求

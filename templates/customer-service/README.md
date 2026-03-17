# Customer Service Bot Template

智能客服机器人模板，支持常见问题解答、工单创建和转人工服务。

## 功能特性

- 🤖 自动回答常见问题 (FAQ)
- 🎫 智能工单创建与管理
- 🔄 无缝转接人工客服
- 📊 对话满意度调查
- 🌐 多语言支持

## 快速开始

```bash
# 使用模板初始化项目
uhorse init --template customer-service

# 配置渠道（以 Telegram 为例）
export TELEGRAM_BOT_TOKEN="your_token"

# 启动服务
uhorse start
```

## 配置说明

### 渠道配置

```toml
[channels.telegram]
bot_token = "${TELEGRAM_BOT_TOKEN}"
enabled = true

[channels.dingtalk]
app_key = "your_app_key"
app_secret = "your_app_secret"
agent_id = 123456
enabled = true
```

### 知识库配置

```toml
[customer_service]
knowledge_base = "./data/faq.json"
ticket_system = "jira"  # jira, zendesk, freshdesk
escalation_threshold = 3  # 连续3次无法回答则转人工
```

## 技能列表

| 技能 | 描述 |
|------|------|
| `faq_search` | FAQ 知识库搜索 |
| `create_ticket` | 创建客服工单 |
| `check_status` | 查询工单状态 |
| `escalate` | 转接人工客服 |
| `satisfaction` | 满意度调查 |

## 目录结构

```
customer-service/
├── config.toml           # 配置文件
├── data/
│   ├── faq.json         # FAQ 知识库
│   └── uhorse.db        # 数据库
├── skills/
│   ├── faq_search/      # FAQ 搜索技能
│   ├── create_ticket/   # 工单创建技能
│   └── escalate/        # 转人工技能
└── workspace/
    └── default/
        ├── SOUL.md      # 客服机器人人设
        └── MEMORY.md    # 长期记忆
```

## FAQ 知识库格式

```json
{
  "categories": [
    {
      "name": "账户相关",
      "questions": [
        {
          "q": "如何修改密码？",
          "a": "请进入 设置 > 账户安全 > 修改密码...",
          "keywords": ["密码", "修改密码", "改密码"]
        }
      ]
    }
  ]
}
```

## 扩展开发

### 添加自定义技能

```bash
# 创建新技能
uhorse skill create check_order

# 编辑技能定义
vim skills/check_order/SKILL.md
```

### 集成第三方工单系统

支持以下工单系统集成：
- Jira Service Desk
- Zendesk
- Freshdesk
- 自定义 API

## 最佳实践

1. **知识库维护**: 定期更新 FAQ，添加高频问题
2. **转人工策略**: 设置合理的转人工阈值
3. **满意度跟踪**: 收集用户反馈持续优化
4. **多渠道统一**: 保持各渠道回答一致性

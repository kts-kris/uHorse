# General Purpose Template

通用助手模板，提供基础的对话能力和常用工具。

## 功能特性

- 💬 智能对话
- 🔍 信息查询
- 📝 文本处理
- 🧮 计算工具
- ⏰ 时间日期
- 🌐 网络搜索

## 快速开始

```bash
uhorse init --template general
uhorse start
```

## 技能列表

| 技能 | 描述 |
|------|------|
| `calculator` | 数学计算 |
| `datetime` | 时间日期查询 |
| `weather` | 天气查询 |
| `translate` | 翻译 |
| `search` | 网络搜索 |
| `reminder` | 提醒设置 |

## 目录结构

```
general/
├── config.toml
├── data/
│   └── uhorse.db
├── skills/
│   ├── calculator/
│   ├── datetime/
│   └── weather/
└── workspace/
    └── default/
        ├── SOUL.md
        └── MEMORY.md
```

## 配置说明

```toml
[general]
# 机器人名称
bot_name = "小助手"

# 问候语
greeting = "你好！我是智能助手，有什么可以帮你的？"

# 启用技能
enabled_skills = ["calculator", "datetime", "weather"]
```

## 扩展开发

### 添加自定义技能

1. 创建技能目录
```bash
mkdir -p skills/my_skill
```

2. 编写 SKILL.md
```markdown
# My Skill

## 描述
自定义技能说明

## 参数
- param1: 参数1说明

## 示例
输入: xxx
输出: xxx
```

3. 在配置中启用
```toml
[general]
enabled_skills = ["calculator", "my_skill"]
```

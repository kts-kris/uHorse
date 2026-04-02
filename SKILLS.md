# uHorse Skill 指南

本文档描述当前 `v4.4.0` 主线下 Skill 的开发、运行时加载与在线安装边界。

## 📖 目录

- [概述](#概述)
- [运行时加载与来源层级](#运行时加载与来源层级)
- [在线安装 Skill](#在线安装-skill)
- [SKILL.md 格式](#skillmd-格式)
- [技能目录结构](#技能目录结构)
- [开发自定义技能](#开发自定义技能)
- [当前限制](#当前限制)

---

## 概述

uHorse 使用 **SKILL.md** 驱动的技能系统。每个技能都是一个独立的目录，包含：

1. **SKILL.md** - 技能元数据和工具定义
2. **工具实现** - Rust 代码或 WASM 模块
3. **配置文件** - 可选的技能配置

### 技能 vs 工具

| 概念 | 说明 |
|------|------|
| **技能 (Skill)** | 一个完整的功能模块，包含多个相关工具 |
| **工具 (Tool)** | 单个可执行的操作，有明确的输入输出 |

---

## 运行时加载与来源层级

当前 `uhorse-hub` 会从运行时目录加载 Skill，并保留来源元信息：

- `global`
- `tenant`
- `enterprise`
- `department`
- `role`
- `user`

这意味着同名 Skill 可以按不同来源层与 scope 共存，Web API / UI 会通过 `source_layer`、`source_scope` 区分来源。

## 在线安装 Skill

当前 `v4.4.0` 新增两条运行时管理 API：

- `POST /api/v1/skills/install`
- `POST /api/v1/skills/refresh`

最小安装请求示例：

```bash
curl -X POST http://127.0.0.1:8765/api/v1/skills/install \
  -H "Content-Type: application/json" \
  -d '{
    "source_type": "skillhub",
    "package": "demo-skill",
    "download_url": "https://example.com/demo-skill.tar.gz"
  }'
```

如果启用了 DingTalk，还可以通过文本命令安装：

```text
安装技能 <package> <download_url> [version]
install skill <package> <download_url> [version]
```

DingTalk 文本安装入口的权限由 `[[channels.dingtalk.skill_installers]]` 控制：

- 仅限制 DingTalk 文本入口
- 可按 `user_id` / `staff_id` 命中
- 可选叠加 `corp_id` 限制企业范围
- 当前不提供 DingTalk 文本 refresh 命令

---

## SKILL.md 格式

SKILL.md 是技能的核心定义文件，使用 Markdown 格式：

```markdown
# 技能名称

## Description
技能的详细描述，说明功能和用途。

## Version
1.0.0

## Tags
标签1, 标签2, 标签3

## Author
作者名称 <email@example.com>

## Tools

### tool_name_1
工具描述

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "param1": {
      "type": "string",
      "description": "参数1说明"
    },
    "param2": {
      "type": "number",
      "description": "参数2说明",
      "default": 10
    }
  },
  "required": ["param1"]
}
```

**Output Schema:**
```json
{
  "type": "object",
  "properties": {
    "result": {
      "type": "string",
      "description": "执行结果"
    }
  }
}
```

## Permissions
- network:read
- filesystem:write

## Config
```toml
[skill]
timeout = 30
max_retries = 3
```
```

### 必需字段

| 字段 | 说明 |
|------|------|
| `Description` | 技能描述 |
| `Version` | 语义化版本号 |
| `Tools` | 工具定义列表 |

### 可选字段

| 字段 | 说明 |
|------|------|
| `Tags` | 逗号分隔的标签 |
| `Author` | 作者信息 |
| `Permissions` | 所需权限列表 |
| `Config` | TOML 格式的配置 |

---

## 技能目录结构

```
~/.uhorse/skills/
├── weather/                 # 天气技能
│   ├── SKILL.md            # 技能定义
│   ├── src/                # Rust 源代码
│   │   └── lib.rs
│   ├── Cargo.toml
│   └── config.toml         # 技能配置
│
├── calculator/              # 计算器技能
│   ├── SKILL.md
│   └── wasm/               # WASM 实现
│       └── calculator.wasm
│
└── web_search/             # 网页搜索技能
    ├── SKILL.md
    └── config.toml
```

---

## 内置技能

uHorse 内置以下技能：

### 1. Calculator

基础数学计算。

```markdown
# Calculator

## Description
执行基础数学运算

## Tools

### calculate
计算数学表达式

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "expression": {
      "type": "string",
      "description": "数学表达式，如 '2 + 2 * 3'"
    }
  },
  "required": ["expression"]
}
```
```

### 2. Time

时间相关操作。

```markdown
# Time

## Description
获取和操作时间信息

## Tools

### current_time
获取当前时间

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "timezone": {
      "type": "string",
      "description": "时区，如 'Asia/Shanghai'",
      "default": "UTC"
    },
    "format": {
      "type": "string",
      "description": "时间格式",
      "default": "%Y-%m-%d %H:%M:%S"
    }
  }
}
```
```

### 3. Text Search

文本搜索工具。

```markdown
# Text Search

## Description
在文本中搜索模式

## Tools

### search
搜索文本模式

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "text": {
      "type": "string",
      "description": "要搜索的文本"
    },
    "pattern": {
      "type": "string",
      "description": "搜索模式（支持正则）"
    },
    "case_sensitive": {
      "type": "boolean",
      "default": false
    }
  },
  "required": ["text", "pattern"]
}
```
```

---

## 开发自定义技能

### 步骤 1：创建技能目录

```bash
mkdir -p ~/.uhorse/skills/my_skill
cd ~/.uhorse/skills/my_skill
```

### 步骤 2：创建 SKILL.md

```bash
cat > SKILL.md << 'EOF'
# My Skill

## Description
我的自定义技能

## Version
1.0.0

## Tags
custom, example

## Tools

### my_tool
工具描述

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "input": {
      "type": "string",
      "description": "输入参数"
    }
  },
  "required": ["input"]
}
```
EOF
```

### 步骤 3：实现工具

**方式 A：Rust 实现**

```rust
// src/lib.rs
use uhorse_tool::{Tool, ToolResult};

pub struct MyTool;

impl Tool for MyTool {
    fn name(&self) -> &str {
        "my_tool"
    }

    async fn execute(&self, input: serde_json::Value) -> ToolResult {
        let input = input["input"].as_str().unwrap_or("");
        Ok(serde_json::json!({
            "result": format!("Processed: {}", input)
        }))
    }
}

uhorse_tool::register!(MyTool);
```

**方式 B：WASM 实现**

```rust
// wasm/src/lib.rs
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn my_tool(input: &str) -> String {
    format!("Processed: {}", input)
}
```

**方式 C：外部命令**

```toml
# config.toml
[tool.my_tool]
type = "command"
command = "/path/to/script.sh"
timeout = 30
```

### 步骤 4：测试技能

```bash
# 使用 uHorse CLI 测试
uhorse skill test my_skill --tool my_tool --input '{"input": "test"}'
```

### 步骤 5：注册技能

```bash
# 在配置中启用
echo 'skills = ["my_skill"]' >> ~/.uhorse/config.toml
```

---

## 当前限制

- 在线安装当前只接受 `source_type = "skillhub"`
- 安装时会拒绝覆盖已存在的 Skill 目录
- DingTalk 文本入口只支持 install，不支持 refresh
- `skill_installers` 不是全局 RBAC，只限制 DingTalk 文本安装入口

---

## 最佳实践

### 1. 单一职责

每个工具应该只做一件事，并且做好。

### 2. 清晰的文档

在 SKILL.md 中提供详细的描述和示例。

### 3. 输入验证

使用 JSON Schema 严格验证输入。

### 4. 错误处理

提供有意义的错误信息。

### 5. 权限最小化

只请求必要的权限。

### 6. 版本管理

使用语义化版本，遵循 semver 规范。

---

## 调试技巧

### 启用调试日志

```bash
RUST_LOG=uhorse_tool=debug uhorse run
```

### 查看技能状态

```bash
uhorse skill list
uhorse skill show my_skill
```

### 测试工具

```bash
uhorse skill test my_skill --tool my_tool --input '{"test": "value"}'
```

---

## 参考资源

- [JSON Schema 规范](https://json-schema.org/)
- [MCP 协议文档](https://modelcontextprotocol.io/)
- [Rust WASM 指南](https://rustwasm.github.io/docs/book/)

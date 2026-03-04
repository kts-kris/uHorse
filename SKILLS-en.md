# Skill Development Guide

This document explains how to develop custom skills for uHorse.

## 📖 Table of Contents

- [Overview](#overview)
- [SKILL.md Format](#skillmd-format)
- [Skill Directory Structure](#skill-directory-structure)
- [Built-in Skills](#built-in-skills)
- [Developing Custom Skills](#developing-custom-skills)
- [Publishing Skills](#publishing-skills)

---

## Overview

uHorse uses a **SKILL.md** driven skill system. Each skill is an independent directory containing:

1. **SKILL.md** - Skill metadata and tool definitions
2. **Tool Implementation** - Rust code or WASM module
3. **Configuration** - Optional skill configuration

### Skills vs Tools

| Concept | Description |
|---------|-------------|
| **Skill** | A complete functional module containing multiple related tools |
| **Tool** | A single executable operation with clear inputs and outputs |

---

## SKILL.md Format

SKILL.md is the core definition file for a skill, using Markdown format:

```markdown
# Skill Name

## Description
Detailed description of the skill's functionality and purpose.

## Version
1.0.0

## Tags
tag1, tag2, tag3

## Author
Author Name <email@example.com>

## Tools

### tool_name_1
Tool description

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "param1": {
      "type": "string",
      "description": "Parameter 1 description"
    },
    "param2": {
      "type": "number",
      "description": "Parameter 2 description",
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
      "description": "Execution result"
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

### Required Fields

| Field | Description |
|-------|-------------|
| `Description` | Skill description |
| `Version` | Semantic version number |
| `Tools` | Tool definition list |

### Optional Fields

| Field | Description |
|-------|-------------|
| `Tags` | Comma-separated tags |
| `Author` | Author information |
| `Permissions` | Required permissions list |
| `Config` | TOML format configuration |

---

## Skill Directory Structure

```
~/.uhorse/skills/
├── weather/                 # Weather skill
│   ├── SKILL.md            # Skill definition
│   ├── src/                # Rust source code
│   │   └── lib.rs
│   ├── Cargo.toml
│   └── config.toml         # Skill configuration
│
├── calculator/              # Calculator skill
│   ├── SKILL.md
│   └── wasm/               # WASM implementation
│       └── calculator.wasm
│
└── web_search/             # Web search skill
    ├── SKILL.md
    └── config.toml
```

---

## Built-in Skills

uHorse includes the following built-in skills:

### 1. Calculator

Basic mathematical calculations.

```markdown
# Calculator

## Description
Perform basic mathematical operations

## Tools

### calculate
Calculate mathematical expression

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "expression": {
      "type": "string",
      "description": "Mathematical expression, e.g., '2 + 2 * 3'"
    }
  },
  "required": ["expression"]
}
```
```

### 2. Time

Time-related operations.

```markdown
# Time

## Description
Get and manipulate time information

## Tools

### current_time
Get current time

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "timezone": {
      "type": "string",
      "description": "Timezone, e.g., 'America/New_York'",
      "default": "UTC"
    },
    "format": {
      "type": "string",
      "description": "Time format",
      "default": "%Y-%m-%d %H:%M:%S"
    }
  }
}
```
```

### 3. Text Search

Text pattern search tool.

```markdown
# Text Search

## Description
Search for patterns in text

## Tools

### search
Search text patterns

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "text": {
      "type": "string",
      "description": "Text to search in"
    },
    "pattern": {
      "type": "string",
      "description": "Search pattern (supports regex)"
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

## Developing Custom Skills

### Step 1: Create Skill Directory

```bash
mkdir -p ~/.uhorse/skills/my_skill
cd ~/.uhorse/skills/my_skill
```

### Step 2: Create SKILL.md

```bash
cat > SKILL.md << 'EOF'
# My Skill

## Description
My custom skill

## Version
1.0.0

## Tags
custom, example

## Tools

### my_tool
Tool description

**Input Schema:**
```json
{
  "type": "object",
  "properties": {
    "input": {
      "type": "string",
      "description": "Input parameter"
    }
  },
  "required": ["input"]
}
```
EOF
```

### Step 3: Implement Tool

**Method A: Rust Implementation**

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

**Method B: WASM Implementation**

```rust
// wasm/src/lib.rs
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn my_tool(input: &str) -> String {
    format!("Processed: {}", input)
}
```

**Method C: External Command**

```toml
# config.toml
[tool.my_tool]
type = "command"
command = "/path/to/script.sh"
timeout = 30
```

### Step 4: Test Skill

```bash
# Test using uHorse CLI
uhorse skill test my_skill --tool my_tool --input '{"input": "test"}'
```

### Step 5: Register Skill

```bash
# Enable in configuration
echo 'skills = ["my_skill"]' >> ~/.uhorse/config.toml
```

---

## Publishing Skills

### Publish to Skill Marketplace

1. Create a GitHub repository
2. Add `uhorse-skill` label
3. Submit to [uHorse Skills Registry](https://github.com/uhorse/skills)

### Install Community Skills

```bash
# Install from GitHub
uhorse skill install github:user/uhorse-skill-name

# Install from local path
uhorse skill install /path/to/skill
```

---

## Best Practices

### 1. Single Responsibility

Each tool should do one thing and do it well.

### 2. Clear Documentation

Provide detailed descriptions and examples in SKILL.md.

### 3. Input Validation

Use JSON Schema to strictly validate inputs.

### 4. Error Handling

Provide meaningful error messages.

### 5. Minimal Permissions

Only request necessary permissions.

### 6. Version Management

Use semantic versioning, follow semver specification.

---

## Debugging Tips

### Enable Debug Logging

```bash
RUST_LOG=uhorse_tool=debug uhorse run
```

### View Skill Status

```bash
uhorse skill list
uhorse skill show my_skill
```

### Test Tool

```bash
uhorse skill test my_skill --tool my_tool --input '{"test": "value"}'
```

---

## Resources

- [JSON Schema Specification](https://json-schema.org/)
- [MCP Protocol Documentation](https://modelcontextprotocol.io/)
- [Rust WASM Guide](https://rustwasm.github.io/docs/book/)

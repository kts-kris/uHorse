//! Skill editor handlers

use super::{SkillMeta, SkillContent, ValidationError};
use std::fs;
use std::path::Path;

/// List all skills in the skills directory
pub fn list_skills(skills_dir: &Path) -> Result<Vec<SkillMeta>, std::io::Error> {
    let mut skills = Vec::new();

    if !skills_dir.exists() {
        return Ok(skills);
    }

    for entry in fs::read_dir(skills_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            let skill_file = path.join("SKILL.md");
            if skill_file.exists() {
                let name = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                let metadata = fs::metadata(&skill_file)?;
                let created_at = metadata.created()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64);

                let updated_at = metadata.modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64);

                // Parse skill description from SKILL.md
                let description = fs::read_to_string(&skill_file)
                    .ok()
                    .and_then(|content| {
                        content.lines()
                            .find(|line| line.starts_with("# "))
                            .map(|line| line.trim_start_matches("# ").to_string())
                    });

                skills.push(SkillMeta {
                    name,
                    path: path.to_string_lossy().to_string(),
                    description,
                    version: None,
                    created_at,
                    updated_at,
                });
            }
        }
    }

    // Sort by name
    skills.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(skills)
}

/// Get skill content
pub fn get_skill(skills_dir: &Path, name: &str) -> Result<SkillContent, std::io::Error> {
    let skill_path = skills_dir.join(name);
    let skill_file = skill_path.join("SKILL.md");

    if !skill_file.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Skill '{}' not found", name),
        ));
    }

    let content = fs::read_to_string(&skill_file)?;

    let metadata = fs::metadata(&skill_file)?;
    let created_at = metadata.created()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64);

    let updated_at = metadata.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64);

    let description = content.lines()
        .find(|line| line.starts_with("# "))
        .map(|line| line.trim_start_matches("# ").to_string());

    // Try to parse schema from skill
    let schema = parse_skill_schema(&content);

    // Validate and get errors
    let validation_errors = super::validator::validate_skill_content(&content)
        .unwrap_or_default();

    Ok(SkillContent {
        meta: SkillMeta {
            name: name.to_string(),
            path: skill_path.to_string_lossy().to_string(),
            description,
            version: None,
            created_at,
            updated_at,
        },
        content,
        schema,
        validation_errors,
    })
}

/// Update skill content
pub fn update_skill(
    skills_dir: &Path,
    name: &str,
    content: &str,
) -> Result<(), std::io::Error> {
    let skill_path = skills_dir.join(name);
    let skill_file = skill_path.join("SKILL.md");

    if !skill_file.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Skill '{}' not found", name),
        ));
    }

    // Create backup
    let backup_file = skill_file.with_extension("md.bak");
    fs::copy(&skill_file, &backup_file)?;

    // Write new content
    fs::write(&skill_file, content)?;

    Ok(())
}

/// Create new skill
pub fn create_skill(
    skills_dir: &Path,
    name: &str,
    template: Option<&str>,
) -> Result<SkillContent, std::io::Error> {
    let skill_path = skills_dir.join(name);
    let skill_file = skill_path.join("SKILL.md");

    if skill_file.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("Skill '{}' already exists", name),
        ));
    }

    // Create skill directory
    fs::create_dir_all(&skill_path)?;

    // Get template content
    let content = if let Some(template_name) = template {
        get_template(template_name)
            .map(|t| t.content)
            .unwrap_or_else(|| default_skill_template(name))
    } else {
        default_skill_template(name)
    };

    // Write skill file
    fs::write(&skill_file, &content)?;

    get_skill(skills_dir, name)
}

/// Delete skill
pub fn delete_skill(skills_dir: &Path, name: &str) -> Result<(), std::io::Error> {
    let skill_path = skills_dir.join(name);

    if !skill_path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Skill '{}' not found", name),
        ));
    }

    fs::remove_dir_all(&skill_path)?;

    Ok(())
}

/// Skill template
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillTemplate {
    pub name: String,
    pub description: String,
    pub content: String,
}

/// List available templates
pub fn list_templates() -> Vec<SkillTemplate> {
    vec![
        SkillTemplate {
            name: "basic".to_string(),
            description: "Basic skill template with minimal structure".to_string(),
            content: include_str!("templates/basic.md").to_string(),
        },
        SkillTemplate {
            name: "api".to_string(),
            description: "API calling skill template".to_string(),
            content: include_str!("templates/api.md").to_string(),
        },
        SkillTemplate {
            name: "calculator".to_string(),
            description: "Calculator skill template".to_string(),
            content: include_str!("templates/calculator.md").to_string(),
        },
        SkillTemplate {
            name: "search".to_string(),
            description: "Web search skill template".to_string(),
            content: include_str!("templates/search.md").to_string(),
        },
    ]
}

/// Get template by name
pub fn get_template(name: &str) -> Option<SkillTemplate> {
    list_templates().into_iter().find(|t| t.name == name)
}

/// Default skill template
fn default_skill_template(name: &str) -> String {
    format!(
        r#"# {}

A brief description of what this skill does.

## Description

Detailed explanation of the skill's purpose and behavior.

## Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| input | string | Yes | The input to process |

## Examples

### Example 1: Basic usage

```json
{{
  "input": "example input"
}}
```

Expected output:
```json
{{
  "result": "processed output"
}}
```

## Implementation

### Rust

```rust
// Implementation code here
```

## Notes

- Any important notes or caveats
- Edge cases to consider
"#,
        name
    )
}

/// Parse skill schema from content
fn parse_skill_schema(content: &str) -> Option<serde_json::Value> {
    // Look for JSON schema in code blocks
    let in_schema = false;
    let mut schema_lines = Vec::new();

    for line in content.lines() {
        if line.contains("```json") && line.contains("schema") {
            // Start of schema block
            continue;
        }
    }

    if schema_lines.is_empty() {
        None
    } else {
        serde_json::from_str(&schema_lines.join("\n")).ok()
    }
}

//! Skill content validator

use super::ValidationError;

/// Validate skill content
pub fn validate_skill_content(content: &str) -> Result<Vec<ValidationError>, String> {
    let mut errors = Vec::new();

    // Check required sections
    validate_required_sections(content, &mut errors);

    // Check markdown structure
    validate_markdown_structure(content, &mut errors);

    // Check parameter table
    validate_parameter_table(content, &mut errors);

    // Check examples
    validate_examples(content, &mut errors);

    Ok(errors)
}

/// Validate required sections exist
fn validate_required_sections(content: &str, errors: &mut Vec<ValidationError>) {
    let required_sections = ["Description", "Parameters", "Examples"];
    let mut found_sections = std::collections::HashSet::new();

    for line in content.lines() {
        for section in &required_sections {
            if line.starts_with("## ") && line.contains(section) {
                found_sections.insert(*section);
            }
        }
    }

    for section in &required_sections {
        if !found_sections.contains(section) {
            errors.push(ValidationError {
                line: None,
                column: None,
                message: format!("Missing required section: {}", section),
                severity: "warning".to_string(),
            });
        }
    }
}

/// Validate markdown structure
fn validate_markdown_structure(content: &str, errors: &mut Vec<ValidationError>) {
    let mut line_num = 0;
    let mut has_title = false;

    for line in content.lines() {
        line_num += 1;

        // Check for title (first # heading)
        if line.starts_with("# ") && !has_title {
            has_title = true;
            let title = line.trim_start_matches("# ").trim();
            if title.is_empty() {
                errors.push(ValidationError {
                    line: Some(line_num),
                    column: Some(1),
                    message: "Skill title is empty".to_string(),
                    severity: "error".to_string(),
                });
            }
        }

        // Check for proper heading hierarchy
        if line.starts_with("###") && !line.starts_with("### ") {
            errors.push(ValidationError {
                line: Some(line_num),
                column: Some(1),
                message: "Invalid heading format: missing space after ###".to_string(),
                severity: "warning".to_string(),
            });
        }
    }

    if !has_title {
        errors.push(ValidationError {
            line: Some(1),
            column: None,
            message: "Skill must have a title (# heading)".to_string(),
            severity: "error".to_string(),
        });
    }
}

/// Validate parameter table format
fn validate_parameter_table(content: &str, errors: &mut Vec<ValidationError>) {
    let mut in_params_section = false;
    let mut found_table_header = false;
    let mut line_num = 0;

    for line in content.lines() {
        line_num += 1;

        if line.starts_with("## Parameters") {
            in_params_section = true;
            continue;
        }

        if line.starts_with("## ") && in_params_section {
            // End of parameters section
            break;
        }

        if in_params_section && line.starts_with("| ") {
            if !found_table_header {
                // First table row should be header
                if !line.contains("Name") || !line.contains("Type") {
                    errors.push(ValidationError {
                        line: Some(line_num),
                        column: Some(1),
                        message: "Parameter table should have Name and Type columns".to_string(),
                        severity: "info".to_string(),
                    });
                }
                found_table_header = true;
            }
        }
    }

    if in_params_section && !found_table_header {
        errors.push(ValidationError {
            line: None,
            column: None,
            message: "Parameters section has no table".to_string(),
            severity: "info".to_string(),
        });
    }
}

/// Validate JSON examples
fn validate_examples(content: &str, errors: &mut Vec<ValidationError>) {
    let mut in_code_block = false;
    let mut is_json_block = false;
    let mut block_start_line = 0;
    let mut block_content = String::new();
    let mut line_num = 0;

    for line in content.lines() {
        line_num += 1;

        if line.starts_with("```") {
            if !in_code_block {
                in_code_block = true;
                is_json_block = line.contains("json");
                block_start_line = line_num;
                block_content.clear();
            } else {
                // End of code block
                if is_json_block && !block_content.is_empty() {
                    if let Err(e) = serde_json::from_str::<serde_json::Value>(&block_content) {
                        errors.push(ValidationError {
                            line: Some(block_start_line),
                            column: None,
                            message: format!("Invalid JSON in example: {}", e),
                            severity: "error".to_string(),
                        });
                    }
                }
                in_code_block = false;
                is_json_block = false;
            }
        } else if in_code_block {
            block_content.push_str(line);
            block_content.push('\n');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_valid_skill() {
        let content = r#"
# Test Skill

Description here.

## Description

This is a test skill.

## Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| input | string | Yes | Input to process |

## Examples

```json
{
  "input": "test"
}
```
"#;
        let errors = validate_skill_content(content).unwrap();
        let errors: Vec<_> = errors.into_iter().filter(|e| e.severity == "error").collect();
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_missing_title() {
        let content = "No title here";
        let errors = validate_skill_content(content).unwrap();
        assert!(errors.iter().any(|e| e.message.contains("title")));
    }

    #[test]
    fn test_validate_invalid_json() {
        let content = r#"
# Test

## Examples

```json
{ invalid json }
```
"#;
        let errors = validate_skill_content(content).unwrap();
        assert!(errors.iter().any(|e| e.message.contains("Invalid JSON")));
    }
}

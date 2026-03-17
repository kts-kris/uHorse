# Calculator Skill

Perform mathematical calculations.

## Description

This skill performs mathematical calculations including basic arithmetic, trigonometry, and common mathematical functions.

## Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| expression | string | Yes | Mathematical expression to evaluate |
| precision | number | No | Number of decimal places. Default: 10 |

## Supported Operations

- Basic: `+`, `-`, `*`, `/`, `%`, `^`
- Functions: `sin`, `cos`, `tan`, `sqrt`, `abs`, `log`, `ln`, `exp`
- Constants: `pi`, `e`
- Parentheses for grouping

## Returns

| Field | Type | Description |
|-------|------|-------------|
| result | number | The calculation result |
| expression | string | The original expression |

## Examples

### Example 1: Basic arithmetic

Input:
```json
{
  "expression": "2 + 3 * 4"
}
```

Output:
```json
{
  "result": 14,
  "expression": "2 + 3 * 4"
}
```

### Example 2: Using functions

Input:
```json
{
  "expression": "sqrt(16) + sin(pi/2)"
}
```

Output:
```json
{
  "result": 5,
  "expression": "sqrt(16) + sin(pi/2)"
}
```

### Example 3: With precision

Input:
```json
{
  "expression": "1/3",
  "precision": 2
}
```

Output:
```json
{
  "result": 0.33,
  "expression": "1/3"
}
```

## Error Handling

| Error Code | Description |
|------------|-------------|
| SYNTAX_ERROR | Expression syntax is invalid |
| DIVISION_BY_ZERO | Division by zero attempted |
| UNKNOWN_FUNCTION | Function not recognized |
| DOMAIN_ERROR | Value outside function domain |

## Security Notes

- Expression length limited to 1000 characters
- No variable assignment allowed
- Execution timeout: 1 second

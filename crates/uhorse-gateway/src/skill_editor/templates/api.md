# API Call Skill

Make HTTP requests to external APIs.

## Description

This skill enables making HTTP requests to external REST APIs. Supports GET, POST, PUT, DELETE methods with custom headers and request bodies.

## Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| url | string | Yes | The API endpoint URL |
| method | string | No | HTTP method (GET, POST, PUT, DELETE). Default: GET |
| headers | object | No | Custom headers as key-value pairs |
| body | any | No | Request body for POST/PUT requests |
| timeout | number | No | Request timeout in seconds. Default: 30 |

## Returns

| Field | Type | Description |
|-------|------|-------------|
| status | number | HTTP status code |
| headers | object | Response headers |
| body | any | Response body |

## Examples

### Example 1: GET request

Input:
```json
{
  "url": "https://api.example.com/users/123",
  "method": "GET",
  "headers": {
    "Authorization": "Bearer token123"
  }
}
```

Output:
```json
{
  "status": 200,
  "headers": {
    "content-type": "application/json"
  },
  "body": {
    "id": 123,
    "name": "John Doe"
  }
}
```

### Example 2: POST request

Input:
```json
{
  "url": "https://api.example.com/users",
  "method": "POST",
  "headers": {
    "Content-Type": "application/json"
  },
  "body": {
    "name": "Jane Doe",
    "email": "jane@example.com"
  }
}
```

## Error Handling

| Error Code | Description |
|------------|-------------|
| TIMEOUT | Request timed out |
| NETWORK_ERROR | Network connection failed |
| INVALID_URL | URL format is invalid |
| HTTP_ERROR | Server returned error status |

## Security Considerations

- Only HTTPS URLs are allowed in production
- Sensitive data in headers should be marked as secret
- Rate limiting may apply

## Configuration

```toml
[skill.api]
allowed_domains = ["api.example.com"]  # Restrict to specific domains
default_timeout = 30
max_retries = 3
```

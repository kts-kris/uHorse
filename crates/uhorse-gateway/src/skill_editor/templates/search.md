# Web Search Skill

Search the web for information.

## Description

This skill performs web searches and returns relevant results. Supports multiple search engines and result filtering.

## Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| query | string | Yes | Search query |
| limit | number | No | Maximum number of results. Default: 5 |
| engine | string | No | Search engine (google, bing, duckduckgo). Default: google |
| safe_search | boolean | No | Enable safe search. Default: true |

## Returns

| Field | Type | Description |
|-------|------|-------------|
| results | array | List of search results |
| total | number | Total number of results available |

### Result Object

| Field | Type | Description |
|-------|------|-------------|
| title | string | Page title |
| url | string | Page URL |
| snippet | string | Text snippet from the page |
| source | string | Source domain |

## Examples

### Example 1: Basic search

Input:
```json
{
  "query": "Rust programming language"
}
```

Output:
```json
{
  "results": [
    {
      "title": "Rust Programming Language",
      "url": "https://www.rust-lang.org/",
      "snippet": "A language empowering everyone to build reliable and efficient software.",
      "source": "rust-lang.org"
    }
  ],
  "total": 1000
}
```

### Example 2: With limit

Input:
```json
{
  "query": "machine learning tutorials",
  "limit": 3,
  "engine": "duckduckgo"
}
```

## Error Handling

| Error Code | Description |
|------------|-------------|
| RATE_LIMITED | Search rate limit exceeded |
| NO_RESULTS | No results found |
| ENGINE_ERROR | Search engine error |

## Configuration

```toml
[skill.search]
api_key = "${SEARCH_API_KEY}"
default_engine = "google"
daily_limit = 100
```

## Notes

- Results are cached for 1 hour
- API key required for production use
- Respect rate limits to avoid blocking

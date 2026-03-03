# Security Policy 安全策略

## Reporting Security Issues 报告安全问题

If you discover a security vulnerability in uHorse,please report it responsibly.
如果你在 uHorse 中发现安全漏洞，请负责任地报告。

### ⚠️ Do NOT create a public GitHub issue
### ⚠️ 不要公开创建 GitHub Issue

Publicly disclosing security vulnerabilities can put users at risk.
公开披露安全漏洞可能会危及用户。

### 🔒 Instead, please report via email
### 🔒 请通过电子邮件报告

Send an email to: **security@uhorse.dev**

请发送邮件至: **security@uhorse.dev**

Include the following information (请包含以下信息):

1. **Subject Line**: Start with `[SECURITY]` (标题行: 以 `[SECURITY]` 开头)
2. **Description**: Detailed description of the vulnerability (描述: 漏洞的详细描述)
3. **Steps to Reproduce**: How to trigger the vulnerability (复现步骤: 如何触发该漏洞)
4. **Potential Impact**: What could an attacker do? (潜在影响: 攻击者能做什么？)
5. **Suggested Fix**: If you have ideas for fixing it (建议修复: 如果你有修复想法)

## What to Expect 期待什么

- We will acknowledge your email within 48 hours
- 我们会在 48 小时内确认收到你的邮件
- We will investigate and keep you updated on our progress
- 我们会调查并随时向你通报进展
- We may ask for additional information if needed
- 如果需要，我们可能会要求提供更多信息
- We will credit you in our security advisories (if desired)
- 我们会在安全公告中感谢你（如果你愿意）

## Safe Harbor 安全港

uHorse has a responsible disclosure process. We commit to:
uHorse 有负责任的披露流程。我们承诺：

- Not taking legal action against researchers who report vulnerabilities in good faith
- 不对善意报告漏洞的研究人员采取法律行动
- Working with you to understand and resolve the issue
- 与你合作理解和解决问题
- Providing public credit for the discovery (with your permission)
- 公开感谢发现（在获得你的许可后）

## Security Best Practices 安全最佳实践

When using uHorse,follow these best practices:
使用 uHorse 时，请遵循这些最佳实践：

### Secrets Management 密钥管理

- **Never** commit secrets to version control
- **永远不要** 将密钥提交到版本控制
- Use environment variables for sensitive configuration
- 使用环境变量存储敏感配置
- Rotate API keys and tokens regularly
- 定期轮换 API 密钥和令牌

### Network Security 网络安全

- Use HTTPS for all communications
- 所有通信使用 HTTPS
- Validate webhook signatures
- 验证 webhook 签名
- Implement IP allowlisting where possible
- 尽可能实现 IP 白名单

### Access Control 访问控制

- Use the built-in JWT authentication
- 使用内置的 JWT 认证
- Enable device pairing for new clients
- 为新客户端启用设备配对
- Review audit logs regularly
- 定期审查审计日志

## Supported Versions 支持的版本

We provide security updates for:
我们为以下版本提供安全更新：

| Version | Support Status |
|---------|---------------|
| 1.x     | ✅ Active support |
| < 1.0   | ❌ End of life |

Thank you for helping keep uHorse secure! 🛡️
感谢你帮助保持 uHorse 的安全！ 🛡️

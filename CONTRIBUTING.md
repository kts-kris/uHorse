# 贡献指南

感谢你考虑为 uHorse 做贡献！

## 🌟 贡献方式

### 报告 Bug

如果你发现了 bug，请通过 [GitHub Issues](https://github.com/uhorse/uhorse-rs/issues) 提交报告。提交前请：

1. 搜索现有的 issues，确认没有重复
2. 使用 Bug Report 模板
3. 提供详细的复现步骤

### 提出新功能

1. 先在 Issues 中讨论你的想法
2. 使用 Feature Request 模板
3. 等待维护者反馈后再开始实现

### 提交代码

#### 1. Fork 并克隆仓库

```bash
git clone https://github.com/YOUR_USERNAME/uhorse-rs
cd uhorse-rs
```

#### 2. 创建分支

```bash
git checkout -b feature/your-feature-name
# 或
git checkout -b fix/your-bug-fix
```

#### 3. 开发环境设置

```bash
# 安装 Rust（如果还没有）
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 安装开发工具
cargo install cargo-nextest cargo-audit

# 编译项目
cargo build

# 运行测试
cargo nextest run
```

#### 4. 编码规范

**格式化**:
```bash
cargo fmt --all
```

**Clippy 检查**:
```bash
cargo clippy --all-targets --all-features -- -D warnings
```

**测试**:
```bash
cargo nextest run
cargo test --doc
```

#### 5. 提交代码

我们使用 [Conventional Commits](https://www.conventionalcommits.org/) 规范：

```
<type>(<scope>): <subject>

<body>

<footer>
```

**类型**:
- `feat`: 新功能
- `fix`: Bug 修复
- `docs`: 文档更新
- `style`: 代码格式（不影响逻辑）
- `refactor`: 重构
- `perf`: 性能优化
- `test`: 测试相关
- `chore`: 构建/工具相关

**示例**:
```
feat(channel): add support for DingTalk

- Implement DingTalkChannel struct
- Add message parsing for DingTalk events
- Add configuration support in wizard

Closes #123
```

#### 6. 推送并创建 PR

```bash
git push origin feature/your-feature-name
```

然后在 GitHub 上创建 Pull Request。

## 📋 PR 检查清单

- [ ] 代码已格式化 (`cargo fmt`)
- [ ] 通过 clippy 检查
- [ ] 所有测试通过
- [ ] 添加了必要的测试
- [ ] 更新了相关文档
- [ ] 提交信息遵循规范

## 🏗️ 项目结构

```
uhorse/
├── crates/
│   ├── uhorse-core/         # 核心类型和 Trait
│   ├── uhorse-gateway/      # HTTP/WebSocket 网关
│   ├── uhorse-channel/      # 通道适配器
│   ├── uhorse-agent/        # 智能体管理
│   ├── uhorse-llm/          # LLM 抽象层
│   ├── uhorse-tool/         # 工具执行
│   ├── uhorse-storage/      # 存储层
│   ├── uhorse-security/     # 安全层
│   ├── uhorse-scheduler/    # 调度器
│   ├── uhorse-config/       # 配置管理
│   └── uhorse-bin/          # 二进制入口
├── .github/                 # GitHub 配置
└── docs/                    # 文档
```

## 🔐 安全问题

如果你发现安全漏洞，请**不要**公开创建 Issue。请发送邮件到 security@uhorse.dev。

## 📜 许可证

通过贡献代码，你同意你的代码将按照 MIT 或 Apache-2.0 许可证授权。

## 💬 联系方式

- GitHub Issues: 用于 bug 报告和功能请求
- GitHub Discussions: 用于一般讨论

---

再次感谢你的贡献！ 🎉

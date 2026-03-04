# Contributing Guide

Thank you for considering contributing to uHorse!

## 🌟 Ways to Contribute

### Report Bugs

If you find a bug, please report it via [GitHub Issues](https://github.com/kts-kris/uHorse/issues). Before submitting:

1. Search existing issues to confirm no duplicates
2. Use the Bug Report template
3. Provide detailed reproduction steps

### Propose New Features

1. First discuss your idea in Issues
2. Use the Feature Request template
3. Wait for maintainer feedback before implementing

### Submit Code

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit changes (`git commit -m 'feat: add amazing feature'`)
4. Push to branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

---

## 🛠️ Development Setup

### Prerequisites

- Rust 1.70+
- cargo-nextest (for testing)
- cargo-watch (for hot reload)

### Getting Started

```bash
# Clone your fork
git clone https://github.com/YOUR_USERNAME/uHorse
cd uHorse

# Add upstream remote
git remote add upstream https://github.com/kts-kris/uHorse

# Install development tools
cargo install cargo-nextest cargo-watch

# Build
cargo build

# Run tests
cargo nextest run

# Run clippy
cargo clippy -- -D warnings

# Format check
cargo fmt -- --check
```

### Hot Reload Development

```bash
# Auto-rebuild on changes
cargo watch -x run

# Auto-run tests on changes
cargo watch -x "nextest run"
```

---

## 📝 Coding Standards

### Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add new feature
fix: fix bug in module
docs: update documentation
refactor: refactor code structure
test: add tests
chore: update dependencies
```

### Code Style

- Run `cargo fmt` before committing
- Address all `cargo clippy` warnings
- Add documentation comments for public APIs
- Write tests for new functionality

### Branch Naming

- `feature/description` - New features
- `fix/description` - Bug fixes
- `refactor/description` - Code refactoring
- `docs/description` - Documentation updates

---

## 🧪 Testing

### Run Tests

```bash
# Run all tests
cargo nextest run

# Run specific test
cargo nextest run test_name

# Run with coverage
cargo tarpaulin
```

### Write Tests

- Place unit tests in `#[cfg(test)]` modules
- Place integration tests in `tests/` directory
- Use descriptive test names

```rust
#[test]
fn should_parse_config_correctly() {
    // Arrange
    let config = "...";

    // Act
    let result = parse_config(config);

    // Assert
    assert!(result.is_ok());
}
```

---

## 📋 Pull Request Guidelines

### Before Submitting

- [ ] Code compiles without warnings
- [ ] All tests pass
- [ ] Code is formatted (`cargo fmt`)
- [ ] Clippy passes (`cargo clippy`)
- [ ] Documentation is updated if needed
- [ ] Commit messages follow conventions

### PR Template

```markdown
## Description
Brief description of changes

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change
- [ ] Documentation update

## Testing
- [ ] Tests added/updated
- [ ] All tests pass

## Checklist
- [ ] Code follows style guidelines
- [ ] Self-review completed
- [ ] Documentation updated
```

---

## 🔒 Security

For security issues, please email **security@uhorse.dev** instead of opening a public issue.

---

## 📞 Contact

- GitHub Issues: For bug reports and feature requests
- Discussions: For questions and general discussion
- Email: For security issues only

---

Thank you for contributing! 🎉

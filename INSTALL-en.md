# uHorse Installation Guide

## Table of Contents

- [System Requirements](#system-requirements)
- [Installation Methods](#installation-methods)
- [Verify Installation](#verify-installation)
- [Troubleshooting](#troubleshooting)

---

## System Requirements

### Minimum Requirements

- **Operating System**: Linux, macOS, or Windows (WSL2)
- **Rust**: 1.70 or higher
- **Memory**: 512 MB available
- **Disk**: 100 MB available space

### Recommended

- **Memory**: 2 GB or more
- **Disk**: 1 GB or more (including logs and data)
- **CPU**: 2 cores or more

---

## Installation Methods

### Method 1: Download Binary (Recommended)

Download pre-built binaries from [GitHub Releases](https://github.com/kts-kris/uHorse/releases):

**Linux (x86_64)**
```bash
curl -LO https://github.com/kts-kris/uHorse/releases/latest/download/uhorse-x86_64-unknown-linux-gnu.tar.gz
tar xzf uhorse-x86_64-unknown-linux-gnu.tar.gz
sudo mv uhorse /usr/local/bin/
```

**macOS (Apple Silicon)**
```bash
curl -LO https://github.com/kts-kris/uHorse/releases/latest/download/uhorse-aarch64-apple-darwin.tar.gz
tar xzf uhorse-aarch64-apple-darwin.tar.gz
sudo mv uhorse /usr/local/bin/
```

**macOS (Intel)**
```bash
curl -LO https://github.com/kts-kris/uHorse/releases/latest/download/uhorse-x86_64-apple-darwin.tar.gz
tar xzf uhorse-x86_64-apple-darwin.tar.gz
sudo mv uhorse /usr/local/bin/
```

**Windows**
```powershell
# Download from browser or use curl
curl -LO https://github.com/kts-kris/uHorse/releases/latest/download/uhorse-x86_64-pc-windows-msvc.zip
Expand-Archive uhorse-x86_64-pc-windows-msvc.zip
```

### Method 2: Build from Source

```bash
# Clone repository
git clone https://github.com/kts-kris/uHorse
cd uHorse

# Build release binary
cargo build --release

# Binary location: ./target/release/uhorse
sudo cp ./target/release/uhorse /usr/local/bin/
```

### Method 3: One-Click Install

```bash
# Clone repository
git clone https://github.com/kts-kris/uHorse
cd uHorse

# Run install script (checks dependencies, compiles, configures)
./install.sh
```

### Method 4: Docker

```bash
# Using docker-compose (recommended)
docker-compose up -d

# Or using docker directly
docker build -t uhorse .
docker run -d -p 8080:8080 -v ./data:/app/data uhorse
```

---

## Configuration

### Interactive Wizard

After installation, run the configuration wizard:

```bash
uhorse wizard
```

The wizard will guide you through:
- 📡 Server address and port
- 💾 Database (SQLite or PostgreSQL)
- 📱 Channel credentials (select channels you need)
- 🤖 LLM configuration (OpenAI, Anthropic, Gemini...)
- 🔒 Security settings (JWT secret, token expiration)

### Manual Configuration

Create `config.toml`:

```toml
[server]
host = "127.0.0.1"
port = 8080

[database]
type = "sqlite"
path = "./data/uhorse.db"

[channels]
enabled = ["telegram"]

[channels.telegram]
bot_token = "your_bot_token"

[llm]
provider = "openai"
api_key = "sk-..."
model = "gpt-4"

[security]
jwt_secret = "your-secret-key"
token_expiry = "24h"
```

---

## Verify Installation

### Health Check

```bash
# Liveness check
curl http://localhost:8080/health/live

# Readiness check
curl http://localhost:8080/health/ready
```

### View Metrics

```bash
curl http://localhost:8080/metrics
```

### Check Version

```bash
uhorse --version
```

---

## Troubleshooting

### Build Errors

**Problem**: OpenSSL not found
```bash
# Ubuntu/Debian
sudo apt-get install libssl-dev pkg-config

# Fedora/RHEL
sudo dnf install openssl-devel pkg-config

# macOS (Homebrew)
brew install openssl pkg-config
```

**Problem**: Rust version too old
```bash
# Update Rust
rustup update stable
```

### Runtime Errors

**Problem**: Port already in use
```bash
# Check port usage
lsof -i :8080

# Use different port
uhorse --port 8081
```

**Problem**: Database permission denied
```bash
# Fix permissions
chmod 755 ./data
chmod 644 ./data/uhorse.db
```

### Docker Issues

**Problem**: Container won't start
```bash
# Check logs
docker-compose logs uhorse

# Rebuild container
docker-compose down
docker-compose build --no-cache
docker-compose up -d
```

---

## Next Steps

- [Configuration Guide](CONFIG.md) - Detailed configuration options
- [Channel Setup](CHANNELS.md) - Configure messaging channels
- [API Reference](API.md) - REST API documentation
- [Deployment Guide](deployments/DEPLOYMENT.md) - Production deployment

---

## Need Help?

- 📖 [Documentation](https://github.com/kts-kris/uHorse#documentation)
- 💬 [Discussions](https://github.com/kts-kris/uHorse/discussions)
- 🐛 [Report Bug](https://github.com/kts-kris/uHorse/issues)

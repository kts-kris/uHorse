# uHorse Installation Guide

This document only describes the **current `v4.1.2` Hub-Node mainline** installation path that matches the repository as it exists today.

The recommended path is:

- build `uhorse-hub` and `uhorse-node`
- optionally build or download `uhorse-node-desktop`
- generate `hub.toml` and `node.toml`
- start Hub and Node separately

> Note: the repository still contains the legacy `uhorse` monolithic binary and helper scripts such as `install.sh` and `quick-setup.sh`, but those are not the primary path documented here.

## Table of Contents

- [System Requirements](#system-requirements)
- [Install from Source](#install-from-source)
- [Optional: package Node Desktop](#optional-package-node-desktop)
- [Optional: build the legacy monolithic binary](#optional-build-the-legacy-monolithic-binary)
- [Installation Verification](#installation-verification)
- [About the helper scripts](#about-the-helper-scripts)
- [Troubleshooting](#troubleshooting)
- [Next Steps](#next-steps)

---

## System Requirements

### Minimum

- **OS**: Linux, macOS, or Windows via WSL2
- **Rust**: `1.78+`
- **Memory**: at least `512 MB`
- **Disk**: at least `200 MB`

### Common dependencies

- `cargo`
- `openssl`
- `pkg-config`

### Recommended

- current Rust stable toolchain
- `2 GB+` memory
- network access between Node and Hub

---

## Install from Source

This is the recommended path for the current repository.

### 1. Clone the repository

```bash
git clone https://github.com/uhorse/uhorse-rs
cd uhorse-rs
```

### 2. Build the mainline binaries

```bash
cargo build --release -p uhorse-hub -p uhorse-node -p uhorse-node-desktop
```

Primary outputs:

- `target/release/uhorse-hub`
- `target/release/uhorse-node`
- `target/release/uhorse-node-desktop`

If you do not want to build locally, you can also use the mainstream-platform `uhorse-hub` and `uhorse-node-desktop` archives from GitHub Release / nightly.

### 3. Generate default configs

```bash
./target/release/uhorse-hub init --output hub.toml
./target/release/uhorse-node init --output node.toml
```

### 4. Adjust the configs

For a minimal local roundtrip you usually only need:

- `hub.toml` for Hub host / port / scheduler fields
- `node.toml` for node name / workspace / Hub WebSocket URL

See [CONFIG-en.md](CONFIG-en.md) for the actual config structure. If you want to validate Node Desktop notification mirroring to DingTalk, Hub also needs `channels.dingtalk.notification_bindings` in addition to the DingTalk credentials.

### 5. Start Hub and Node

Terminal 1:

```bash
./target/release/uhorse-hub --config hub.toml --log-level info
```

Terminal 2:

```bash
./target/release/uhorse-node --config node.toml --log-level info
```

---

## Optional: package Node Desktop

If you want to ship the desktop client instead of only running the local host API, use the built-in packaging script.

The fixed `v4.1.2` delivery boundary is: **`bin + web` archive delivery, `desktop-smoke.sh`, and CI / release / nightly artifacts**. This means the current mainline already covers archive packaging and smoke validation, but it does **not** include native `.app/.dmg`, code signing, notarization, or installers.

Use the built-in packaging script:

```bash
./scripts/package-node-desktop.sh
```

Default outputs:

- `target/node-desktop-package/uhorse-node-desktop-<version>-<target>/bin/uhorse-node-desktop`
- `target/node-desktop-package/uhorse-node-desktop-<version>-<target>/web/`
- matching `.tar.gz` or `.zip` archive

To verify the packaged host API and static assets together, run:

```bash
./scripts/desktop-smoke.sh
```

This smoke currently validates:

- the Node Desktop host API
- static asset serving
- SPA route fallback

It does not imply that native installers, platform distribution, or signing pipelines are complete.

---

## Optional: build the legacy monolithic binary

The workspace still contains the `uhorse` binary target. If you need it for legacy scripts or historical compatibility checks, build it separately:

```bash
cargo build --release -p uhorse
```

Output:

- `target/release/uhorse`

> The current README, config docs, and deployment docs are centered on `uhorse-hub` + `uhorse-node`.

---

## Installation Verification

### 1. Check the binaries

```bash
./target/release/uhorse-hub --help
./target/release/uhorse-node --help
```

### 2. Verify Node workspace access

```bash
./target/release/uhorse-node check --workspace .
```

### 3. Check Hub after startup

```bash
curl http://127.0.0.1:8765/api/health
curl http://127.0.0.1:8765/metrics
curl http://127.0.0.1:8765/api/nodes
```

### 4. Run the verified local roundtrip test

```bash
cargo test -p uhorse-hub test_local_hub_node_roundtrip_file_exists -- --nocapture
cargo test -p uhorse-hub test_local_hub_node_roundtrip_file_write -- --nocapture
```

These tests start real:

- Hub
- WebSocket server
- Node
- a file existence roundtrip task
- a real file write roundtrip, including on-disk persistence and structured `file_operation` output

### 5. Verify the Node Desktop `v4.1.2` archive boundary

If you are validating the `v4.1.2` Node Desktop deliverable, also run:

```bash
./scripts/package-node-desktop.sh
./scripts/desktop-smoke.sh
```

The acceptance bar is that the archive can be produced and the host API / static asset smoke passes, not that a native `.app/.dmg` or installer exists.

---

## About the helper scripts

These scripts still exist in the repository root:

- `install.sh`
- `quick-setup.sh`
- `start.sh`
- `stop.sh`

They are now aligned to the current Hub-Node mainline:

- `install.sh`: builds `uhorse-hub` / `uhorse-node` and generates minimal `hub.toml` / `node.toml`
- `quick-setup.sh`: creates a minimal local setup
- `start.sh` / `stop.sh`: manage the local `uhorse-hub` process only

If you want to:

- verify the local Hub-Node roundtrip
- configure DingTalk Stream or mirror Node Desktop notifications to DingTalk
- configure LLMs or a custom model provider
- deploy Hub on a server and Node on a workstation

prefer the `uhorse-hub` and `uhorse-node` commands from this document.

---

## Troubleshooting

### Rust version is too old

```bash
rustc --version
rustup update
```

### Missing OpenSSL / pkg-config

**macOS**

```bash
brew install openssl pkg-config
```

**Ubuntu / Debian**

```bash
sudo apt-get update
sudo apt-get install -y libssl-dev pkg-config
```

### Only one binary was built

Make sure the build command includes both packages:

```bash
cargo build --release -p uhorse-hub -p uhorse-node
```

### Node cannot connect to Hub

Check:

- the Hub port in `hub.toml`
- `connection.hub_url` in `node.toml`
- the `/ws` path

Example:

```toml
[connection]
hub_url = "ws://127.0.0.1:8765/ws"
```

---

## Next Steps

- [README-en.md](README-en.md): project overview
- [CONFIG-en.md](CONFIG-en.md): actual config structure and examples
- [LOCAL_SETUP.md](LOCAL_SETUP.md): local Hub-Node startup guide
- [TESTING.md](TESTING.md): build, test, and roundtrip verification
- [deployments/DEPLOYMENT_V4.md](deployments/DEPLOYMENT_V4.md): v4 Hub-Node deployment guide

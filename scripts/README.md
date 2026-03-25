# uHorse 主线脚本说明

本目录脚本围绕 **当前 Hub + Node 主线** 组织，不再默认验证旧单体 `uhorse`、旧 `/health/live`、旧 `/health/ready`。

## 可用脚本

### `dev.sh`

热重载启动 `uhorse-hub`：

```bash
./scripts/dev.sh
```

默认行为：

- 启动 `cargo watch`
- 运行 `uhorse-hub --host 127.0.0.1 --port 8765`
- 健康检查地址为 `http://127.0.0.1:8765/api/health`

### `start.sh`

前台启动当前主线 Hub：

```bash
./scripts/start.sh
```

该脚本等价于仓库根目录 `./run.sh`。

### `quick-test.sh`

快速验证当前主线关键链路：

```bash
./scripts/quick-test.sh
```

覆盖内容：

- `uhorse-hub` / `uhorse-node` release 编译
- 真实 `test_local_hub_node_roundtrip_file_exists`
- `uhorse-node check --workspace .`
- Hub Docker 构建
- Docker 内 `GET /api/health` 与 `GET /api/nodes` smoke

### `test.sh`

执行更完整的主线回归：

```bash
./scripts/test.sh
```

覆盖内容：

- `uhorse-node-runtime` 包级测试
- `uhorse-hub` 包级测试
- roundtrip 回归
- JWT `node_id` 不匹配拒绝回归
- Node workspace 检查
- Hub Docker smoke

## 推荐搭配

```bash
make build
make start
make node-run
make test-quick
make test-full
```

## 参考文档

- `LOCAL_SETUP.md`：Hub / Node 本地启动与联调
- `TESTING.md`：测试矩阵与回归命令
- `INSTALL.md`：安装与最小闭环入口

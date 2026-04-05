#!/bin/bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

HEALTH_URL="http://127.0.0.1:8765/api/health"
NODES_URL="http://127.0.0.1:8765/api/nodes"

pass() { echo "[ok] $1"; }
info() { echo "[info] $1"; }
fail() { echo "[error] $1"; exit 1; }

cleanup() {
    docker compose stop uhorse-hub >/dev/null 2>&1 || true
    docker compose rm -f uhorse-hub >/dev/null 2>&1 || true
}
trap cleanup EXIT

command -v cargo >/dev/null 2>&1 || fail "cargo 未安装"
command -v docker >/dev/null 2>&1 || fail "Docker 未安装"
command -v docker compose >/dev/null 2>&1 || fail "docker compose 未安装"
command -v curl >/dev/null 2>&1 || fail "curl 未安装"
docker info >/dev/null 2>&1 || fail "Docker daemon 未运行"
pass "环境检查通过"

info "编译 Hub + Node release..."
cargo build --release -p uhorse-hub -p uhorse-node >/tmp/uhorse-test-build.log 2>&1
pass "release 编译通过"

info "运行 Node Runtime 测试..."
cargo test -p uhorse-node-runtime >/tmp/uhorse-test-node-runtime.log 2>&1
pass "uhorse-node-runtime 测试通过"

info "运行 Hub 包级测试..."
cargo test -p uhorse-hub >/tmp/uhorse-test-hub.log 2>&1
pass "uhorse-hub 测试通过"

info "运行真实 Hub-Node roundtrip 回归..."
cargo test -p uhorse-hub test_local_hub_node_roundtrip_file_exists -- --nocapture >/tmp/uhorse-test-roundtrip.log 2>&1
pass "roundtrip 回归通过"

info "运行 JWT node_id 拒绝回归..."
cargo test -p uhorse-hub test_local_hub_rejects_node_with_mismatched_auth_token -- --nocapture >/tmp/uhorse-test-auth.log 2>&1
pass "JWT 拒绝回归通过"

info "运行 Agent Loop continuation smoke..."
cargo test -p uhorse-hub test_reply_task_result_records_compaction_and_retries_once -- --nocapture >/tmp/uhorse-test-agent-loop.log 2>&1
pass "Agent Loop continuation smoke 通过"

info "运行 approval wait / resume smoke..."
cargo test -p uhorse-hub test_approval_request_records_wait_metric_and_transcript -- --nocapture >/tmp/uhorse-test-approval-wait.log 2>&1
cargo test -p uhorse-hub test_approve_approval_appends_transcript_event_for_bound_turn -- --nocapture >/tmp/uhorse-test-approval-resume.log 2>&1
pass "approval wait / resume smoke 通过"

info "运行 observability smoke..."
cargo test -p uhorse-observability test_metrics_exporter_returns_prometheus_payload -- --nocapture >/tmp/uhorse-test-observability.log 2>&1
cargo test -p uhorse-backup test_restore_lifecycle_records_audit_events -- --nocapture >/tmp/uhorse-test-restore-audit.log 2>&1
pass "observability smoke 通过"

info "运行 audit smoke..."
cargo test -p uhorse-hub test_approval_decision_records_audit_events -- --nocapture >/tmp/uhorse-test-approval-audit.log 2>&1
cargo test -p uhorse-node-runtime test_dangerous_git_command_records_audit_event -- --nocapture >/tmp/uhorse-test-dangerous-command-audit.log 2>&1
cargo test -p uhorse-node-runtime test_checkpoint_and_restore_record_audit_events -- --nocapture >/tmp/uhorse-test-versioning-audit.log 2>&1
pass "audit smoke 通过"

info "检查当前 workspace 是否可作为 Node 工作区..."
cargo run --quiet --release -p uhorse-node -- check --workspace . >/tmp/uhorse-test-node-check.log 2>&1
pass "Node workspace 检查通过"

info "构建 Hub Docker 镜像..."
docker build -t uhorse-hub:latest -f Dockerfile . >/tmp/uhorse-test-docker.log 2>&1
pass "Docker 镜像构建通过"

info "启动 Docker smoke 环境..."
docker compose up -d uhorse-hub >/tmp/uhorse-test-compose.log 2>&1

for _ in {1..30}; do
    if curl -sf "$HEALTH_URL" | grep -q 'healthy'; then
        pass "Hub /api/health 可访问"
        break
    fi
    sleep 1
done

if ! curl -sf "$HEALTH_URL" | grep -q 'healthy'; then
    docker compose logs --no-log-prefix uhorse-hub | tail -n 80 || true
    fail "Hub /api/health 检查失败"
fi

curl -sf "$NODES_URL" >/tmp/uhorse-test-nodes.json 2>&1
pass "Hub /api/nodes 可访问"

docker compose logs --no-log-prefix uhorse-hub >/tmp/uhorse-test-hub-docker.log 2>&1 || true
pass "Docker 日志已导出"

echo ""
echo "完整测试完成。关键日志："
echo "  /tmp/uhorse-test-build.log"
echo "  /tmp/uhorse-test-node-runtime.log"
echo "  /tmp/uhorse-test-hub.log"
echo "  /tmp/uhorse-test-roundtrip.log"
echo "  /tmp/uhorse-test-auth.log"
echo "  /tmp/uhorse-test-agent-loop.log"
echo "  /tmp/uhorse-test-approval-wait.log"
echo "  /tmp/uhorse-test-approval-resume.log"
echo "  /tmp/uhorse-test-observability.log"
echo "  /tmp/uhorse-test-node-check.log"
echo "  /tmp/uhorse-test-docker.log"
echo "  /tmp/uhorse-test-compose.log"
echo "  /tmp/uhorse-test-hub-docker.log"

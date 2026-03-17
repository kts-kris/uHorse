#!/bin/bash
set -e

# uHorse Playground Entrypoint
# This script initializes the playground environment

echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║                    🦄 uHorse Playground                               ║"
echo "║                    30-Second Quick Start                              ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo ""

# Initialize database if not exists
if [ ! -f /app/data/uhorse.db ]; then
    echo "→ Initializing database..."
    uhorse migrate --config /app/config.toml || true
fi

# Print welcome message
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "  🚀 uHorse is starting..."
echo ""
echo "  📍 Access URLs:"
echo "     • Web UI:    http://localhost:8080"
echo "     • API Docs:  http://localhost:8080/docs"
echo "     • Health:    http://localhost:8080/health/live"
echo ""
echo "  📚 Quick Start Guide:"
echo "     • Send a message to the bot"
echo "     • Try: '你好', '现在几点', '1+1等于几'"
echo ""
echo "  ⚠️  Note: This is a playground environment for testing."
echo "     Data is not persisted between restarts."
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Start uHorse
exec "$@"

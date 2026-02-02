#!/bin/bash
# Trajectory Explorer - startup script
#
# Prerequisites:
#   - Clojure CLI (clj)
#   - npm (for shadow-cljs)
#
# Usage:
#   ./run.sh        # Start server only (uses pre-built JS)
#   ./run.sh dev    # Start with hot-reload development

set -e

cd "$(dirname "$0")"

if [ "$1" = "dev" ]; then
    echo "Starting in development mode..."
    echo "1. Starting shadow-cljs watch..."
    npx shadow-cljs watch app &
    SHADOW_PID=$!
    sleep 3
    echo "2. Starting Clojure server..."
    clj -M:server
    kill $SHADOW_PID 2>/dev/null
else
    echo "Starting Trajectory Explorer..."
    echo "Building ClojureScript..."
    npx shadow-cljs release app
    echo ""
    echo "Starting server..."
    clj -M:server
fi

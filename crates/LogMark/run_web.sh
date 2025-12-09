#!/bin/bash
# LogMark Web Build and Serve Script
# This script builds and serves the LogMark web application using trunk

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "🚀 Building and serving LogMark Web..."
echo "📂 Working directory: $SCRIPT_DIR"

# Check if trunk is installed
if ! command -v trunk &> /dev/null; then
    echo "❌ trunk is not installed. Installing..."
    cargo install trunk
fi

# Check if wasm32 target is installed
if ! rustup target list --installed | grep -q wasm32-unknown-unknown; then
    echo "📦 Installing wasm32-unknown-unknown target..."
    rustup target add wasm32-unknown-unknown
fi

# Clean previous builds if requested
if [ "$1" == "--clean" ]; then
    echo "🧹 Cleaning previous builds..."
    rm -rf dist/
    cargo clean -p logmark
fi

# Skip wasm-opt to avoid bulk memory issues with newer Rust versions
export TRUNK_TOOLS_WASM_OPT=""

# Build and serve
echo "🔨 Building and serving..."
echo "   Open http://localhost:8080 in your browser"
echo ""

# Check if release mode is requested
if [ "$1" == "--release" ] || [ "$2" == "--release" ]; then
    echo "📦 Building in release mode..."
    trunk serve --release --open
else
    echo "🔧 Building in debug mode (faster builds)..."
    trunk serve --open
fi

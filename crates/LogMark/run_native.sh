#!/bin/bash
# LogMark Native Build and Run Script
# This script builds and runs the LogMark native desktop application

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "🚀 Building and running LogMark Native..."
echo "📂 Working directory: $SCRIPT_DIR"

# Clean previous builds if requested
if [ "$1" == "--clean" ]; then
    echo "🧹 Cleaning previous builds..."
    cargo clean -p logmark
fi

# Build mode
BUILD_MODE="--release"
if [ "$1" == "--debug" ]; then
    BUILD_MODE=""
    echo "🔧 Building in debug mode..."
else
    echo "🔧 Building in release mode..."
fi

# Build and run
echo "🔨 Building..."
cargo build $BUILD_MODE -p logmark --bin logmark_app

echo "▶️  Running LogMark..."
if [ -z "$BUILD_MODE" ]; then
    cargo run -p logmark --bin logmark_app
else
    cargo run --release -p logmark --bin logmark_app
fi

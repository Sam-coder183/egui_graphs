#!/bin/bash
# Build LogMark for WebAssembly using Trunk
# Requirements: trunk (cargo install trunk)

set -e

echo "Building LogMark for WebAssembly..."

# Check if trunk is installed
if ! command -v trunk &> /dev/null; then
    echo "trunk not found. Installing..."
    cargo install trunk
fi

# Build with trunk
trunk build --release

echo "Build complete! Files are in ./dist/"
echo "To serve locally: trunk serve"

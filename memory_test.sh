#!/bin/bash
# Build and check binary sizes

echo "Building release binary..."
cargo build --release 2>&1 | tee build.log

if [ $? -eq 0 ]; then
    echo -e "\n=== Binary Size Analysis ==="
    cargo size --release -- -A

    echo -e "\n=== Symbol Sizes (top 20 largest) ==="
    cargo nm --release -- --print-size --size-sort | tail -20
else
    echo "Build failed - check build.log for errors"
fi

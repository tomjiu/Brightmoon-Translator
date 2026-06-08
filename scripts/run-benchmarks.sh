#!/bin/bash
# Moon Translator Performance Benchmark Runner
# Usage: ./scripts/run-benchmarks.sh [suite]

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BENCH_DIR="$SCRIPT_DIR/../src-tauri"
SUITE=$1

cd "$BENCH_DIR"

if [ -z "$SUITE" ]; then
    echo "Running all benchmarks..."
    echo ""

    echo "[1/3] Translation Engine Benchmarks"
    echo "===================================="
    cargo bench --bench translation_bench
    echo ""

    echo "[2/3] Cache System Benchmarks"
    echo "=============================="
    cargo bench --bench cache_bench
    echo ""

    echo "[3/3] OCR Processing Benchmarks"
    echo "================================"
    cargo bench --bench ocr_bench
    echo ""

    echo "All benchmarks complete!"
    echo "Reports saved to: $BENCH_DIR/target/criterion/"
else
    echo "Running $SUITE benchmarks..."
    cargo bench --bench "$SUITE"
    echo ""
    echo "Benchmark complete!"
    echo "Reports saved to: $BENCH_DIR/target/criterion/$SUITE"
fi

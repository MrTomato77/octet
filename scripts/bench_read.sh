#!/bin/sh
# Adapter-read benchmark (sealed). Last line: {"primary": median ms, ...}; lower is better.
cd "$(dirname "$0")/.." || exit 1
out=$(cargo test -q --release -p octet read_bench -- --ignored --nocapture --test-threads=1 2>/dev/null | grep '^{"primary"' | tail -n 1)
[ -n "$out" ] || { echo "benchmark failed" >&2; exit 1; }
echo "$out"

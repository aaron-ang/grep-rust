#!/usr/bin/env bash
set -euo pipefail

if ! command -v hyperfine >/dev/null 2>&1; then
  echo "hyperfine is not installed"
  echo "macOS: brew install hyperfine"
  echo "Ubuntu/Debian: sudo apt install hyperfine"
  exit 1
fi

pattern="${1:-matched_line_\\d+}"
input_file="${2:-bench/data.txt}"

if [[ ! -f "$input_file" ]]; then
  echo "benchmark input not found: $input_file"
  echo "generate one with: ./scripts/gen-bench-data.sh $input_file"
  exit 1
fi

cargo build --release >/dev/null

hyperfine \
  --warmup 2 \
  --shell bash \
  "target/release/grep-rust -E '$pattern' '$input_file'" \
  "grep -E '$pattern' '$input_file'"

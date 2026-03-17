#!/usr/bin/env bash
set -euo pipefail

out_file="${1:-bench/data.txt}"
line_count="${2:-200000}"

mkdir -p "$(dirname "$out_file")"
: > "$out_file"

for i in $(seq 1 "$line_count"); do
  if (( i % 10 == 0 )); then
    printf 'log=%06d level=INFO user=user_%06d code=%04d message=matched_line_%06d\n' \
      "$i" "$i" "$((i % 1000))" "$i" >> "$out_file"
  else
    printf 'log=%06d level=DEBUG user=user_%06d code=%04d message=ordinary_line_%06d\n' \
      "$i" "$i" "$((i % 1000))" "$i" >> "$out_file"
  fi
done

printf 'wrote %s lines to %s\n' "$line_count" "$out_file"

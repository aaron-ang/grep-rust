#!/usr/bin/env bash
set -euo pipefail

generate_logs() {
  local out_file="${1:-bench/data.txt}"
  local line_count="${2:-200000}"

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
}

generate_words() {
  local out_file="${1:-bench/words.txt}"
  local line_count="${2:-100000}"

  mkdir -p "$(dirname "$out_file")"
  : > "$out_file"

  for i in $(seq 1 "$line_count"); do
    if (( i % 3 == 0 )); then
      printf 'cat dog bird\n' >> "$out_file"
    elif (( i % 5 == 0 )); then
      printf 'dog bird cat\n' >> "$out_file"
    else
      printf 'horse cow sheep\n' >> "$out_file"
    fi
  done

  printf 'wrote %s lines to %s\n' "$line_count" "$out_file"
}

generate_nearmiss() {
  local out_file="${1:-bench/nearmiss_small.txt}"
  local line_count="${2:-2000}"

  mkdir -p "$(dirname "$out_file")"
  : > "$out_file"

  local near_miss
  local matched
  near_miss="$(printf 'a%.0s' {1..32})c"
  matched="$(printf 'a%.0s' {1..32})b"

  for i in $(seq 1 "$line_count"); do
    if (( i % 200 == 0 )); then
      printf '%s\n' "$matched" >> "$out_file"
    else
      printf '%s\n' "$near_miss" >> "$out_file"
    fi
  done

  printf 'wrote %s lines to %s\n' "$line_count" "$out_file"
}

generate_backref() {
  local out_file="${1:-bench/backref.txt}"
  local line_count="${2:-50000}"

  mkdir -p "$(dirname "$out_file")"
  : > "$out_file"

  for i in $(seq 1 "$line_count"); do
    if (( i % 4 == 0 )); then
      printf 'token%06d and token%06d\n' "$i" "$i" >> "$out_file"
    else
      printf 'token%06d and other%06d\n' "$i" "$i" >> "$out_file"
    fi
  done

  printf 'wrote %s lines to %s\n' "$line_count" "$out_file"
}

generate_all() {
  generate_logs "bench/data.txt" "200000"
  generate_words "bench/words.txt" "100000"
  generate_nearmiss "bench/nearmiss_small.txt" "2000"
  generate_backref "bench/backref.txt" "50000"
}

case "${1:-all}" in
  all)
    generate_all
    ;;
  logs)
    generate_logs "${2:-bench/data.txt}" "${3:-200000}"
    ;;
  words)
    generate_words "${2:-bench/words.txt}" "${3:-100000}"
    ;;
  nearmiss)
    generate_nearmiss "${2:-bench/nearmiss_small.txt}" "${3:-2000}"
    ;;
  backref)
    generate_backref "${2:-bench/backref.txt}" "${3:-50000}"
    ;;
  *)
    echo "usage: $0 [all|logs|words|nearmiss|backref]" >&2
    exit 1
    ;;
esac

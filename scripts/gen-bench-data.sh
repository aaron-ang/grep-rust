#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
FIXTURES_DIR="${ROOT_DIR}/fixtures/bench"

default_path() {
  printf '%s/%s\n' "${FIXTURES_DIR}" "$1"
}

generate_logs() {
  local out_file="${1:-$(default_path "data.txt")}"
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
  local out_file="${1:-$(default_path "words.txt")}"
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
  local out_file="${1:-$(default_path "nearmiss_small.txt")}"
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
  local out_file="${1:-$(default_path "backref.txt")}"
  local line_count="${2:-50000}"

  mkdir -p "$(dirname "$out_file")"
  : > "$out_file"

  for i in $(seq 1 "$line_count"); do
    case $(( i % 6 )) in
      0)
        printf 'token%06d and token%06d\n' "$i" "$i" >> "$out_file"
        ;;
      1)
        printf 'token%06d and other%06d\n' "$i" "$i" >> "$out_file"
        ;;
      2)
        printf 'token%06d-%06d and token%06d-%06d\n' "$i" "$i" "$i" "$i" >> "$out_file"
        ;;
      3)
        printf 'token%06d-%06d and token%06d-%06d\n' "$i" "$i" "$i" "$((i + 1))" >> "$out_file"
        ;;
      4)
        printf 'abca-abca\n' >> "$out_file"
        ;;
      5)
        printf 'abca-abcb\n' >> "$out_file"
        ;;
    esac
  done

  printf 'wrote %s lines to %s\n' "$line_count" "$out_file"
}

generate_tree() {
  local out_dir="${1:-$(default_path "tree")}"
  local file_count="${2:-64}"
  local lines_per_file="${3:-2000}"

  rm -rf "$out_dir"
  mkdir -p "$out_dir"

  for i in $(seq 1 "$file_count"); do
    local subdir
    local out_file
    subdir="$(printf '%s/part_%02d' "$out_dir" "$(((i - 1) / 8))")"
    out_file="$(printf '%s/file_%03d.txt' "$subdir" "$i")"
    mkdir -p "$subdir"
    : > "$out_file"

    for j in $(seq 1 "$lines_per_file"); do
      if (( j % 10 == 0 )); then
        printf 'log=%06d level=INFO user=user_%06d code=%04d message=matched_line_%06d\n' \
          "$j" "$j" "$((j % 1000))" "$j" >> "$out_file"
      else
        printf 'log=%06d level=DEBUG user=user_%06d code=%04d message=ordinary_line_%06d\n' \
          "$j" "$j" "$((j % 1000))" "$j" >> "$out_file"
      fi
    done
  done

  printf 'wrote %s files to %s\n' "$file_count" "$out_dir"
}

generate_all() {
  generate_logs "$(default_path "data.txt")" "200000"
  generate_words "$(default_path "words.txt")" "100000"
  generate_nearmiss "$(default_path "nearmiss_small.txt")" "2000"
  generate_backref "$(default_path "backref.txt")" "50000"
  generate_tree "$(default_path "tree")" "64" "2000"
}

case "${1:-all}" in
  all)
    generate_all
    ;;
  logs)
    generate_logs "${2:-$(default_path "data.txt")}" "${3:-200000}"
    ;;
  words)
    generate_words "${2:-$(default_path "words.txt")}" "${3:-100000}"
    ;;
  nearmiss)
    generate_nearmiss "${2:-$(default_path "nearmiss_small.txt")}" "${3:-2000}"
    ;;
  backref)
    generate_backref "${2:-$(default_path "backref.txt")}" "${3:-50000}"
    ;;
  tree)
    generate_tree "${2:-$(default_path "tree")}" "${3:-64}" "${4:-2000}"
    ;;
  *)
    echo "usage: $0 [all|logs|words|nearmiss|backref|tree]" >&2
    exit 1
    ;;
esac

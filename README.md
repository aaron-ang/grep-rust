# grep-rust

A small `grep` implementation in Rust with a custom regex engine, recursive file search, only-match output, ANSI highlighting, and a benchmark workflow against system `grep`.

[![progress-banner](https://backend.codecrafters.io/progress/grep/d5fac5b9-9540-466c-9d43-83878a8eefe6)](https://app.codecrafters.io/users/aaron-ang)

## Features

### CLI

- Search stdin or one or more files
- Recursive directory traversal with `-r`
- Print only matched text with `-o`
- Highlight matches with `--color=always|auto|never`
- Exit with code `0` when at least one match is found, `1` otherwise

### Regex engine

- Literal characters
- Wildcard `.`
- Character classes `\d` and `\w`
- Character groups like `[abc]` and negated groups like `[^abc]`
- Anchors `^` and `$`
- Quantifiers `?`, `+`, `*`, `{n}`, `{n,}`, `{n,m}`
- Grouping and alternation with `(...)` and `|`
- Backreferences like `\1`

## Usage

Run against stdin:

```sh
echo 'The king had 10 children' | cargo run -- -E '\d+'
```

Search files:

```sh
cargo run -- -E 'hello\d+' path/to/file.txt
```

Recursive search:

```sh
cargo run -- -r -E 'hello\d+' src
```

Only matching output:

```sh
echo 'jekyll and hyde' | cargo run -- -o -E '(jekyll|hyde)'
```

Highlighted output:

```sh
echo 'I have 3 apples' | cargo run -- --color=always -E '\d'
```

## Development

Format, lint, and test:

```sh
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## Benchmarking

Install `hyperfine` locally:

```sh
brew install hyperfine
```

Generate a synthetic corpus:

```sh
./scripts/gen-bench-data.sh
```

Run the benchmark and generate an SVG chart:

```sh
./scripts/bench.py
```

By default this benchmarks `grep-rust` against system `grep` on `bench/data.txt` and writes:

```text
bench/benchmark.svg
```

You can override the regex pattern and input file:

```sh
./scripts/bench.py 'user_\d+' bench/data.txt
```

## Benchmark Chart

![Benchmark comparison](bench/benchmark.svg)

## Optimization Notes

The current implementation is competitive with system `grep` mainly because it avoids unnecessary work:

- Regexes are compiled once and reused across lines
- Matching stays on byte spans, with a fast path for patterns that do not need captures
- Fixed literal prefixes and buffered output help cut down scanning and printing overhead

The exact benchmark result is still workload-dependent, so some patterns will benefit more than others.

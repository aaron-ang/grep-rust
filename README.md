# grep-rust

A small `grep` implementation in Rust with a custom regex engine, recursive file search, only-match output, ANSI highlighting, and a benchmark workflow against `grep`, `ripgrep`, and `fastgrep`.

[![progress-banner](https://backend.codecrafters.io/progress/grep/d5fac5b9-9540-466c-9d43-83878a8eefe6)](https://app.codecrafters.io/users/aaron-ang)

## Features

### CLI

- Search stdin or one or more files
- Recursive directory traversal with `-r`
- Parallel file search with `-j, --threads`
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

Recursive search with four worker threads:

```sh
cargo run -- -r -j 4 -E 'hello\d+' src
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

Install the external CLI benchmarking tools:

```sh
brew install hyperfine ripgrep
cargo install cargo-flamegraph
```

`fastgrep` is also expected on `PATH`. If your local install exposes it under a different binary name, set `FASTGREP_BIN=/path/to/that/binary` when running the benchmark script.

Create the Python virtual environment and install plotting dependencies:

```sh
uv venv
uv pip install -r requirements.txt
```

Generate all benchmark corpora:

```sh
uv run scripts/gen-bench-data.sh
```

Run the external CLI benchmark matrix and generate the SVG chart:

```sh
uv run python scripts/bench.py
```

Re-render the chart from the saved benchmark JSON without rerunning `hyperfine`:

```sh
uv run python scripts/plot.py
```

Run the Rust-native Criterion benches:

```sh
cargo bench
```

Generate a flamegraph for a deterministic workload:

```sh
cargo flamegraph --root --output assets/flamegraphs/literal_prefix.svg --bin profile_workload -- --case literal-prefix
```

By default the external matrix benchmarks `grep-rust`, system `grep`, `rg`, and `fastgrep` across a fixed set of patterns:

- `fixtures/bench/data.txt`
  - `matched_line_[0123456789]+`
  - `^log=[0123456789]+ level=INFO`
  - `message=(matched_line|ordinary_line)_[0123456789]+`
- `fixtures/bench/words.txt`
  - `cat dog bird`
  - `^(cat dog bird|dog bird cat)$`
  - `^.+ .+ .+$`
- `fixtures/bench/nearmiss_small.txt`
  - `a+a+a+a+b`
- `fixtures/bench/tree/`
  - `matched_line_[0123456789]+` with recursive `-r`
- `fixtures/bench/backref.txt`
  - `(\w+) and \1`
  - `^(\w+) and \1$`
  - `(\w+)-(\d+) and \1-\2`
  - `^((\w+)-(\d+)) and \1$`
  - `^([abc]+)-\1$`

The benchmark layout is:

```text
scripts/
  bench.py
  plot.py
  gen-bench-data.sh

fixtures/bench/
  data.txt
  words.txt
  nearmiss_small.txt
  backref.txt
  tree/

assets/bench/
  benchmark.json
  benchmark.svg

assets/flamegraphs/
```

`scripts/gen-bench-data.sh` generates all benchmark corpora, and `scripts/bench.py` will call it automatically if any benchmark input is missing. `scripts/bench.py` uses `hyperfine --warmup 3` and otherwise leaves hyperfine's run-count defaults in place.

The benchmark uses multiple corpora instead of a single file because each one stresses a different behavior:

- `fixtures/bench/data.txt` covers literal prefixes, anchors, and alternation on structured log lines
- `fixtures/bench/words.txt` covers simple literal and broad wildcard scans on short repeated phrases
- `fixtures/bench/nearmiss_small.txt` stresses quantified near-miss backtracking
- `fixtures/bench/tree/` measures recursive multi-file throughput, which is where `-j` parallelism matters most
- `fixtures/bench/backref.txt` exercises single, multiple, grouped, and quantified backreference paths in the custom fallback engine

## Benchmark Chart

The generated chart in `assets/bench/benchmark.svg` shows the speedup of `grep-rust` over system `grep`. Values above `1.0x` mean `grep-rust` is faster. The JSON matrix in `assets/bench/benchmark.json` also records the raw timings for `rg` and `fastgrep`.

![Benchmark speedup comparison](assets/bench/benchmark.svg)

## Optimization Notes

The current implementation is competitive with system `grep` mainly because it uses a hybrid search pipeline instead of one matcher for every pattern:

- Pure literals and literal-only alternations are routed to `aho-corasick`
- Regular regexes are compiled with `regex-automata`
- Backreference patterns fall back to the custom matcher instead of slowing down the normal path
- Searches return byte spans directly, so `-o` and highlighting reuse the same match data
- Recursive and multi-file search fan out across worker threads while preserving input file order in the final output
- Output stays buffered, which keeps printing overhead from dominating the benchmark

### Backreference Path

Backreference patterns go through a separate compiled fallback engine. The compiler first tries to recognize a reusable fast path before falling back to the generic VM.

```mermaid
flowchart TD
    A[regex with backreference] --> B[compile BackreferencePlan]
    B --> C{shape detected?}
    C -->|single capture replay| D[SingleCaptureLiteralBackref]
    C -->|two-part replay| E[TwoPartReplayBackref]
    C -->|none| F[generic VM fallback]
    D --> G[find literal separator]
    G --> H[scan repeated atom on left and right]
    H --> I[compare slices directly]
    E --> J[find outer separator]
    J --> K[scan backward for atom 2, middle literal, atom 1]
    K --> L[replay full left slice on the right]
    F --> M[candidate generation from anchor or start predicate]
    M --> N[execute backreference VM]
    I --> O[RegexMatch spans]
    L --> O
    N --> O
```

The current fast paths cover patterns like `(\w+) and \1`, `(\w+)-(\d+) and \1-\2`, and `^((\w+)-(\d+)) and \1$`. Other backreference patterns still run through the VM for correctness.

The exact benchmark result is still workload-dependent, so some patterns will benefit more than others.

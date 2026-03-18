#!/usr/bin/env python3

import json
import shlex
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

from plot import render_plot_from_json


@dataclass(frozen=True)
class BenchmarkCase:
    dataset_label: str
    case_label: str
    pattern: str
    input_file: Path

    @property
    def chart_label(self) -> str:
        return self.case_label


@dataclass(frozen=True)
class BenchmarkResult:
    case: BenchmarkCase
    grep_rust_ms: float
    grep_rust_stddev_ms: float
    grep_ms: float
    grep_stddev_ms: float


DEFAULT_OUTPUT_FILE = Path("bench/benchmark.svg")
DEFAULT_JSON_FILE = Path("bench/benchmark.json")
HYPERFINE_EXPORT_FILE = Path("bench/.hyperfine.json")
BENCH_DATA_FILE = Path("bench/data.txt")
WORDS_DATA_FILE = Path("bench/words.txt")
NEARMISS_DATA_FILE = Path("bench/nearmiss_small.txt")
BACKREF_DATA_FILE = Path("bench/backref.txt")


def fail(message: str) -> None:
    print(message, file=sys.stderr)
    raise SystemExit(1)


def benchmark_cases() -> list[BenchmarkCase]:
    return [
        BenchmarkCase(
            dataset_label="Log corpus",
            case_label="literal prefix",
            pattern="matched_line_[0123456789]+",
            input_file=BENCH_DATA_FILE,
        ),
        BenchmarkCase(
            dataset_label="Log corpus",
            case_label="anchored prefix",
            pattern="^log=[0123456789]+ level=INFO",
            input_file=BENCH_DATA_FILE,
        ),
        BenchmarkCase(
            dataset_label="Log corpus",
            case_label="dense alternation",
            pattern="message=(matched_line|ordinary_line)_[0123456789]+",
            input_file=BENCH_DATA_FILE,
        ),
        BenchmarkCase(
            dataset_label="Word corpus",
            case_label="literal phrase",
            pattern="cat dog bird",
            input_file=WORDS_DATA_FILE,
        ),
        BenchmarkCase(
            dataset_label="Word corpus",
            case_label="anchored alternation",
            pattern="^(cat dog bird|dog bird cat)$",
            input_file=WORDS_DATA_FILE,
        ),
        BenchmarkCase(
            dataset_label="Word corpus",
            case_label="broad wildcard",
            pattern="^.+ .+ .+$",
            input_file=WORDS_DATA_FILE,
        ),
        BenchmarkCase(
            dataset_label="Near-miss corpus",
            case_label="quantified backtracking",
            pattern="a+a+a+a+b",
            input_file=NEARMISS_DATA_FILE,
        ),
        BenchmarkCase(
            dataset_label="Backreference corpus",
            case_label="word repeat",
            pattern=r"(\w+) and \1",
            input_file=BACKREF_DATA_FILE,
        ),
        BenchmarkCase(
            dataset_label="Backreference corpus",
            case_label="anchored repeat",
            pattern=r"^(\w+) and \1$",
            input_file=BACKREF_DATA_FILE,
        ),
        BenchmarkCase(
            dataset_label="Backreference corpus",
            case_label="multiple backrefs",
            pattern=r"(\w+)-(\d+) and \1-\2",
            input_file=BACKREF_DATA_FILE,
        ),
        BenchmarkCase(
            dataset_label="Backreference corpus",
            case_label="group replay",
            pattern=r"^((\w+)-(\d+)) and \1$",
            input_file=BACKREF_DATA_FILE,
        ),
        BenchmarkCase(
            dataset_label="Backreference corpus",
            case_label="quantified capture",
            pattern=r"^([abc]+)-\1$",
            input_file=BACKREF_DATA_FILE,
        ),
    ]


def ensure_benchmark_inputs() -> None:
    if all(
        input_file.is_file()
        for input_file in (
            BENCH_DATA_FILE,
            WORDS_DATA_FILE,
            NEARMISS_DATA_FILE,
            BACKREF_DATA_FILE,
        )
    ):
        return

    subprocess.run(["./scripts/gen-bench-data.sh"], check=True)


def build_command(binary: str, pattern: str, input_file: Path) -> str:
    return (
        f"{shlex.quote(binary)} -E {shlex.quote(pattern)} "
        f"{shlex.quote(str(input_file))} >/dev/null"
    )


def run_case(case: BenchmarkCase) -> BenchmarkResult:
    grep_rust_cmd = build_command("target/release/grep-rust", case.pattern, case.input_file)
    grep_cmd = build_command("grep", case.pattern, case.input_file)
    subprocess.run(
        [
            "hyperfine",
            "--shell",
            "bash",
            "--warmup",
            "3",
            "--export-json",
            str(HYPERFINE_EXPORT_FILE),
            grep_rust_cmd,
            grep_cmd,
        ],
        check=True,
    )
    payload = json.loads(HYPERFINE_EXPORT_FILE.read_text())
    HYPERFINE_EXPORT_FILE.unlink(missing_ok=True)

    grep_rust, grep = payload["results"]
    return BenchmarkResult(
        case=case,
        grep_rust_ms=grep_rust["mean"] * 1000,
        grep_rust_stddev_ms=stddev_ms(grep_rust),
        grep_ms=grep["mean"] * 1000,
        grep_stddev_ms=stddev_ms(grep),
    )


def stddev_ms(result: dict[str, float | None]) -> float:
    stddev = result.get("stddev")
    return 0.0 if stddev is None else stddev * 1000


def write_benchmark_json(json_file: Path, results: list[BenchmarkResult]) -> None:
    payload = {
        "title": "grep-rust vs grep",
        "cases": [
            {
                "dataset_label": result.case.dataset_label,
                "case_label": result.case.case_label,
                "chart_label": result.case.chart_label,
                "pattern": result.case.pattern,
                "input_file": str(result.case.input_file),
                "series": {
                    "grep-rust": {
                        "mean_ms": result.grep_rust_ms,
                        "stddev_ms": result.grep_rust_stddev_ms,
                    },
                    "grep baseline": {
                        "mean_ms": result.grep_ms,
                        "stddev_ms": result.grep_stddev_ms,
                    },
                },
            }
            for result in results
        ],
    }
    json_file.write_text(json.dumps(payload, indent=2) + "\n")


def main() -> None:
    if shutil.which("hyperfine") is None:
        fail(
            "hyperfine is not installed\nmacOS: brew install hyperfine\nUbuntu/Debian: sudo apt install hyperfine"
        )

    DEFAULT_OUTPUT_FILE.parent.mkdir(parents=True, exist_ok=True)
    DEFAULT_JSON_FILE.parent.mkdir(parents=True, exist_ok=True)

    ensure_benchmark_inputs()
    subprocess.run(["cargo", "build", "--release"], check=True)

    results = [run_case(case) for case in benchmark_cases()]
    write_benchmark_json(DEFAULT_JSON_FILE, results)
    render_plot_from_json(DEFAULT_JSON_FILE, DEFAULT_OUTPUT_FILE)

    print(f"Wrote benchmark data to {DEFAULT_JSON_FILE}")
    print(f"Wrote benchmark plot to {DEFAULT_OUTPUT_FILE}")


if __name__ == "__main__":
    main()

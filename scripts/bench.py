#!/usr/bin/env python3

import argparse
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

from plot import write_svg_plot


def build_command(binary: str, pattern: str, input_file: str) -> str:
    return f"{binary} -E '{pattern}' '{input_file}'"


def benchmark_labels() -> list[str]:
    return ["grep-rust", "grep baseline"]


def fail(message: str) -> None:
    print(message, file=sys.stderr)
    raise SystemExit(1)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Benchmark grep-rust against system grep and generate an SVG chart."
    )
    parser.add_argument(
        "-E",
        "--pattern",
        default=r"matched_line_\d+",
        help="regex pattern to benchmark",
    )
    parser.add_argument(
        "-i",
        "--input-file",
        type=Path,
        default=Path("bench/data.txt"),
        help="input file used for benchmarking",
    )
    parser.add_argument(
        "-o",
        "--output-file",
        type=Path,
        default=Path("bench/benchmark.svg"),
        help="output SVG chart path",
    )
    return parser.parse_args()


def main() -> None:
    if shutil.which("hyperfine") is None:
        fail(
            "hyperfine is not installed\nmacOS: brew install hyperfine\nUbuntu/Debian: sudo apt install hyperfine"
        )

    args = parse_args()
    pattern = args.pattern
    input_file = args.input_file
    output_file = args.output_file

    if not input_file.is_file():
        fail(
            f"benchmark input not found: {input_file}\n"
            f"generate one with: ./scripts/gen-bench-data.sh {input_file}"
        )

    output_file.parent.mkdir(parents=True, exist_ok=True)

    subprocess.run(["cargo", "build", "--release"], check=True)

    grep_rust_cmd = build_command("target/release/grep-rust", pattern, str(input_file))
    grep_cmd = build_command("grep", pattern, str(input_file))

    with tempfile.NamedTemporaryFile(suffix=".json") as temp:
        subprocess.run(
            [
                "hyperfine",
                "--warmup",
                "2",
                "--shell",
                "bash",
                "--export-json",
                temp.name,
                grep_rust_cmd,
                grep_cmd,
            ],
            check=True,
        )
        results = json.loads(Path(temp.name).read_text())

    means_ms = [result["mean"] * 1000 for result in results["results"]]
    stddev_ms = [result["stddev"] * 1000 for result in results["results"]]

    write_svg_plot(output_file, benchmark_labels(), means_ms, stddev_ms)

    print(f"Wrote benchmark plot to {output_file}")


if __name__ == "__main__":
    main()

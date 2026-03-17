#!/usr/bin/env python3

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


def main() -> None:
    if shutil.which("hyperfine") is None:
        fail("hyperfine is not installed\nmacOS: brew install hyperfine\nUbuntu/Debian: sudo apt install hyperfine")

    pattern = sys.argv[1] if len(sys.argv) > 1 else r"matched_line_\d+"
    input_file = Path(sys.argv[2]) if len(sys.argv) > 2 else Path("bench/data.txt")
    output_file = Path(sys.argv[3]) if len(sys.argv) > 3 else Path("bench/benchmark.svg")

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

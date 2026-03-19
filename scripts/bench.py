#!/usr/bin/env python3

import json
import os
import shlex
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

from plot import render_plot_from_json

ROOT = Path(__file__).resolve().parents[1]
FIXTURES_DIR = ROOT / "fixtures" / "bench"
ASSETS_DIR = ROOT / "assets" / "bench"
DEFAULT_OUTPUT_FILE = ASSETS_DIR / "benchmark.svg"
DEFAULT_JSON_FILE = ASSETS_DIR / "benchmark.json"
HYPERFINE_EXPORT_FILE = ASSETS_DIR / ".hyperfine.json"
BENCH_DATA_FILE = FIXTURES_DIR / "data.txt"
WORDS_DATA_FILE = FIXTURES_DIR / "words.txt"
NEARMISS_DATA_FILE = FIXTURES_DIR / "nearmiss_small.txt"
BACKREF_DATA_FILE = FIXTURES_DIR / "backref.txt"
TREE_DATA_DIR = FIXTURES_DIR / "tree"
GREP_RUST_BIN = ROOT / "target" / "release" / "grep-rust"


@dataclass(frozen=True)
class BenchmarkCase:
    dataset_label: str
    case_label: str
    pattern: str
    input_path: Path
    recursive: bool = False

    @property
    def chart_label(self):
        return self.case_label


@dataclass(frozen=True)
class ToolResult:
    mean_ms: float
    stddev_ms: float


@dataclass(frozen=True)
class BenchmarkResult:
    case: BenchmarkCase
    series: dict[str, ToolResult | None]
    unsupported_tools: tuple[str, ...] = ()


def fail(message: str):
    print(message, file=sys.stderr)
    raise SystemExit(1)


def benchmark_cases():
    return [
        BenchmarkCase(
            dataset_label="Log corpus",
            case_label="literal prefix",
            pattern="matched_line_[0123456789]+",
            input_path=BENCH_DATA_FILE,
        ),
        BenchmarkCase(
            dataset_label="Log corpus",
            case_label="anchored prefix",
            pattern="^log=[0123456789]+ level=INFO",
            input_path=BENCH_DATA_FILE,
        ),
        BenchmarkCase(
            dataset_label="Log corpus",
            case_label="dense alternation",
            pattern="message=(matched_line|ordinary_line)_[0123456789]+",
            input_path=BENCH_DATA_FILE,
        ),
        BenchmarkCase(
            dataset_label="Word corpus",
            case_label="literal phrase",
            pattern="cat dog bird",
            input_path=WORDS_DATA_FILE,
        ),
        BenchmarkCase(
            dataset_label="Word corpus",
            case_label="anchored alternation",
            pattern="^(cat dog bird|dog bird cat)$",
            input_path=WORDS_DATA_FILE,
        ),
        BenchmarkCase(
            dataset_label="Word corpus",
            case_label="broad wildcard",
            pattern="^.+ .+ .+$",
            input_path=WORDS_DATA_FILE,
        ),
        BenchmarkCase(
            dataset_label="Near-miss corpus",
            case_label="quantified backtracking",
            pattern="a+a+a+a+b",
            input_path=NEARMISS_DATA_FILE,
        ),
        BenchmarkCase(
            dataset_label="Directory corpus",
            case_label="recursive literal prefix",
            pattern="matched_line_[0123456789]+",
            input_path=TREE_DATA_DIR,
            recursive=True,
        ),
        BenchmarkCase(
            dataset_label="Backreference corpus",
            case_label="word repeat",
            pattern=r"(\w+) and \1",
            input_path=BACKREF_DATA_FILE,
        ),
        BenchmarkCase(
            dataset_label="Backreference corpus",
            case_label="anchored repeat",
            pattern=r"^(\w+) and \1$",
            input_path=BACKREF_DATA_FILE,
        ),
        BenchmarkCase(
            dataset_label="Backreference corpus",
            case_label="multiple backrefs",
            pattern=r"(\w+)-(\d+) and \1-\2",
            input_path=BACKREF_DATA_FILE,
        ),
        BenchmarkCase(
            dataset_label="Backreference corpus",
            case_label="group replay",
            pattern=r"^((\w+)-(\d+)) and \1$",
            input_path=BACKREF_DATA_FILE,
        ),
        BenchmarkCase(
            dataset_label="Backreference corpus",
            case_label="quantified capture",
            pattern=r"^([abc]+)-\1$",
            input_path=BACKREF_DATA_FILE,
        ),
    ]


def resolve_required_tool(label: str, command: str, *, env_var: str | None = None):
    if env_var is not None and env_var in os.environ:
        return os.environ[env_var]

    resolved = shutil.which(command)
    if resolved is not None:
        return resolved

    details = f"'{command}' is not available on PATH"
    if env_var is not None:
        details += f" and {env_var} is not set"
    if label == "fastgrep":
        details += (
            "\nSet FASTGREP_BIN if your local install exposes a different binary name."
        )
    fail(details)


def tool_commands():
    return {
        "grep-rust": str(GREP_RUST_BIN),
        "grep baseline": resolve_required_tool("grep", "grep"),
        "ripgrep": resolve_required_tool("rg", "rg"),
        "fastgrep": resolve_required_tool(
            "fastgrep", "fastgrep", env_var="FASTGREP_BIN"
        ),
    }


def ensure_benchmark_inputs():
    if (
        BENCH_DATA_FILE.is_file()
        and WORDS_DATA_FILE.is_file()
        and NEARMISS_DATA_FILE.is_file()
        and BACKREF_DATA_FILE.is_file()
        and TREE_DATA_DIR.is_dir()
    ):
        return

    subprocess.run([str(ROOT / "scripts" / "gen-bench-data.sh")], cwd=ROOT, check=True)


def build_grep_like_command(
    binary: str, pattern: str, input_path: Path, recursive: bool
):
    recursive_flag = "-r " if recursive else ""
    return (
        f"{shlex.quote(binary)} {recursive_flag}-E {shlex.quote(pattern)} "
        f"{shlex.quote(str(input_path))} >/dev/null"
    )


def build_ripgrep_command(binary: str, pattern: str, input_path: Path):
    engine_flag = "-P " if has_backreference(pattern) else ""
    return (
        f"{shlex.quote(binary)} --no-config -uuu {engine_flag}-e {shlex.quote(pattern)} "
        f"{shlex.quote(str(input_path))} >/dev/null"
    )


def has_backreference(pattern: str):
    return any(f"\\{index}" in pattern for index in range(1, 10))


def build_case_commands(case: BenchmarkCase, binaries: dict[str, str]):
    commands = [
        (
            "grep-rust",
            build_grep_like_command(
                binaries["grep-rust"],
                case.pattern,
                case.input_path,
                case.recursive,
            ),
        ),
        (
            "grep baseline",
            build_grep_like_command(
                binaries["grep baseline"],
                case.pattern,
                case.input_path,
                case.recursive,
            ),
        ),
        (
            "ripgrep",
            build_ripgrep_command(binaries["ripgrep"], case.pattern, case.input_path),
        ),
        (
            "fastgrep",
            build_grep_like_command(
                binaries["fastgrep"],
                case.pattern,
                case.input_path,
                case.recursive,
            ),
        ),
    ]
    return commands


def command_supports_case(command: str):
    completed = subprocess.run(
        command,
        cwd=ROOT,
        shell=True,
        executable="bash",
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    return completed.returncode == 0


def run_case(case: BenchmarkCase, binaries: dict[str, str]):
    commands = build_case_commands(case, binaries)
    supported_commands = [
        (label, command)
        for label, command in commands
        if command_supports_case(command)
    ]
    unsupported_tools = tuple(
        label
        for label, _ in commands
        if label not in {name for name, _ in supported_commands}
    )
    subprocess.run(
        [
            "hyperfine",
            "--shell",
            "bash",
            "--warmup",
            "3",
            "--export-json",
            str(HYPERFINE_EXPORT_FILE),
            *[command for _, command in supported_commands],
        ],
        cwd=ROOT,
        check=True,
    )
    payload = json.loads(HYPERFINE_EXPORT_FILE.read_text())
    HYPERFINE_EXPORT_FILE.unlink(missing_ok=True)

    measured_series = {
        label: ToolResult(
            mean_ms=result["mean"] * 1000,
            stddev_ms=stddev_ms(result),
        )
        for (label, _), result in zip(
            supported_commands, payload["results"], strict=True
        )
    }
    series = {
        label: measured_series.get(label)
        for label in ("grep-rust", "grep baseline", "ripgrep", "fastgrep")
    }
    return BenchmarkResult(
        case=case,
        series=series,
        unsupported_tools=unsupported_tools,
    )


def stddev_ms(result: dict[str, float | None]):
    stddev = result.get("stddev")
    return 0.0 if stddev is None else stddev * 1000


def write_benchmark_json(json_file: Path, results: list[BenchmarkResult]):
    payload = {
        "title": "grep-rust external benchmark matrix",
        "cases": [
            {
                "dataset_label": result.case.dataset_label,
                "case_label": result.case.case_label,
                "chart_label": result.case.chart_label,
                "pattern": result.case.pattern,
                "input_file": str(result.case.input_path.relative_to(ROOT)),
                "series": {
                    label: (
                        {
                            "mean_ms": tool_result.mean_ms,
                            "stddev_ms": tool_result.stddev_ms,
                        }
                        if tool_result is not None
                        else None
                    )
                    for label, tool_result in result.series.items()
                },
                "unsupported_tools": list(result.unsupported_tools),
                "speedups": {
                    "over_grep": require_series(result, "grep baseline").mean_ms
                    / require_series(result, "grep-rust").mean_ms,
                    "over_ripgrep": ratio_or_none(result, "ripgrep"),
                    "over_fastgrep": ratio_or_none(result, "fastgrep"),
                },
            }
            for result in results
        ],
    }
    json_file.write_text(json.dumps(payload, indent=2) + "\n")


def require_series(result: BenchmarkResult, label: str):
    tool_result = result.series[label]
    if tool_result is None:
        fail(f"{label} is unexpectedly unavailable for {result.case.case_label}")
    return tool_result


def ratio_or_none(result: BenchmarkResult, label: str):
    tool_result = result.series[label]
    if tool_result is None:
        return None
    return tool_result.mean_ms / require_series(result, "grep-rust").mean_ms


def main():
    if shutil.which("hyperfine") is None:
        fail(
            "hyperfine is not installed\nmacOS: brew install hyperfine\nUbuntu/Debian: sudo apt install hyperfine"
        )

    binaries = tool_commands()
    ASSETS_DIR.mkdir(parents=True, exist_ok=True)

    ensure_benchmark_inputs()
    subprocess.run(["cargo", "build", "--release"], cwd=ROOT, check=True)

    results = [run_case(case, binaries) for case in benchmark_cases()]
    write_benchmark_json(DEFAULT_JSON_FILE, results)
    render_plot_from_json(DEFAULT_JSON_FILE, DEFAULT_OUTPUT_FILE)

    print(f"Wrote benchmark data to {DEFAULT_JSON_FILE.relative_to(ROOT)}")
    print(f"Wrote benchmark plot to {DEFAULT_OUTPUT_FILE.relative_to(ROOT)}")


if __name__ == "__main__":
    main()

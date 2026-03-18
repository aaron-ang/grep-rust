#!/usr/bin/env python3

import json
from dataclasses import dataclass
from pathlib import Path

import matplotlib

matplotlib.use("Agg")

import matplotlib.pyplot as plt


DEFAULT_INPUT_JSON = Path("bench/benchmark.json")
DEFAULT_OUTPUT_FILE = Path("bench/benchmark.svg")


@dataclass(frozen=True)
class BenchmarkPlotData:
    title: str
    case_labels: list[str]
    speedups: list[float]


def write_svg_plot(
    output_file: Path,
    case_labels: list[str],
    speedups: list[float],
    title: str,
) -> None:
    default_width, default_height = plt.rcParams["figure.figsize"]
    longest_label_line = max(
        len(line) for label in case_labels for line in label.splitlines()
    )
    figure_width = max(default_width, len(case_labels) * 1.2, longest_label_line * 0.6)
    figure, axis = plt.subplots(figsize=(figure_width, default_height), layout="constrained")

    base_positions = list(range(len(case_labels)))
    bar_colors = ["#2E8B57" if speedup >= 1.0 else "#C23B22" for speedup in speedups]
    bars = axis.bar(base_positions, speedups, color=bar_colors, width=0.8)

    axis.set_xticks(base_positions, labels=case_labels)
    plt.setp(axis.get_xticklabels(), rotation=0, ha="center", linespacing=0.95)
    axis.set_xlabel("Benchmark case")
    axis.set_ylabel("Speedup (higher is better)")
    axis.set_title(title)
    axis.set_axisbelow(True)
    axis.tick_params(axis="x", pad=4, length=0)
    axis.axhline(1.0, color="0.6", linewidth=1, linestyle="--")

    max_speedup = max(speedups, default=1.0)
    axis.set_ylim(0, max(max_speedup * 1.15, 1.2))

    for bar, speedup in zip(bars, speedups):
        axis.text(
            bar.get_x() + bar.get_width() / 2,
            bar.get_height() + max_speedup * 0.03,
            f"{speedup:.2f}x",
            ha="center",
            va="bottom",
        )

    figure.savefig(output_file, format="svg")
    plt.close(figure)


def load_plot_data(input_json: Path) -> BenchmarkPlotData:
    payload = json.loads(input_json.read_text())
    case_labels = [case["case_label"] for case in payload["cases"]]
    speedups = [
        case["series"]["grep baseline"]["mean_ms"]
        / case["series"]["grep-rust"]["mean_ms"]
        for case in payload["cases"]
    ]
    return BenchmarkPlotData(
        title="Speedup of grep-rust over grep",
        case_labels=case_labels,
        speedups=speedups,
    )


def render_plot_from_json(input_json: Path, output_file: Path) -> None:
    plot_data = load_plot_data(input_json)
    output_file.parent.mkdir(parents=True, exist_ok=True)
    write_svg_plot(
        output_file,
        plot_data.case_labels,
        plot_data.speedups,
        plot_data.title,
    )


def main() -> None:
    render_plot_from_json(DEFAULT_INPUT_JSON, DEFAULT_OUTPUT_FILE)


if __name__ == "__main__":
    main()

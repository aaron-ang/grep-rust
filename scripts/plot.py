#!/usr/bin/env python3

import json
import math
from dataclasses import dataclass
from pathlib import Path

import matplotlib

matplotlib.use("Agg")

import matplotlib.pyplot as plt


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_INPUT_JSON = ROOT / "assets" / "bench" / "benchmark.json"
DEFAULT_OUTPUT_FILE = ROOT / "assets" / "bench" / "benchmark.svg"


@dataclass(frozen=True)
class PlotSeries:
    label: str
    speedups: list[float]


@dataclass(frozen=True)
class BenchmarkPlotData:
    title: str
    case_labels: list[str]
    series: list[PlotSeries]


def wrap_case_label(label: str) -> str:
    if len(label) <= 14 or " " not in label:
        return label

    words = label.split()
    midpoint = len(words) // 2
    best_split = midpoint
    best_delta = float("inf")

    for split in range(1, len(words)):
        left = " ".join(words[:split])
        right = " ".join(words[split:])
        delta = abs(len(left) - len(right))
        if delta < best_delta:
            best_delta = delta
            best_split = split

    return "\n".join((" ".join(words[:best_split]), " ".join(words[best_split:])))


def write_svg_plot(
    output_file: Path,
    case_labels: list[str],
    series: list[PlotSeries],
    title: str,
) -> None:
    wrapped_labels = [wrap_case_label(label) for label in case_labels]
    default_width, default_height = plt.rcParams["figure.figsize"]
    longest_label_line = max(
        len(line) for label in wrapped_labels for line in label.splitlines()
    )
    figure_width = max(
        default_width, len(wrapped_labels) * 1.5, longest_label_line * 0.7
    )
    figure, axis = plt.subplots(
        figsize=(figure_width, default_height * 1.5), layout="constrained"
    )

    base_positions = list(range(len(wrapped_labels)))
    bar_width = 0.8 / len(series)
    offsets = [
        (index - (len(series) - 1) / 2) * bar_width for index in range(len(series))
    ]

    max_speedup = max(
        (
            speedup
            for plot_series in series
            for speedup in plot_series.speedups
            if math.isfinite(speedup)
        ),
        default=1.0,
    )

    for offset, plot_series in zip(offsets, series, strict=True):
        positions = [position + offset for position in base_positions]
        bars = axis.bar(
            positions, plot_series.speedups, width=bar_width, label=plot_series.label
        )
        for bar, speedup in zip(bars, plot_series.speedups, strict=True):
            if not math.isfinite(speedup):
                continue
            axis.text(
                bar.get_x() + bar.get_width() / 2,
                bar.get_height() + max_speedup * 0.02,
                f"{speedup:.2f}",
                ha="center",
                va="bottom",
                fontsize="small",
            )

    axis.set_xticks(base_positions, labels=wrapped_labels)
    plt.setp(axis.get_xticklabels(), rotation=0, ha="center", linespacing=0.95)
    axis.set_xlabel("Benchmark case")
    axis.set_ylabel("Speedup (higher is better)")
    axis.set_title(title)
    axis.set_axisbelow(True)
    axis.tick_params(axis="x", pad=4, length=0)
    axis.axhline(1.0, color="0.6", linewidth=1, linestyle="--")
    axis.set_ylim(0, max(max_speedup * 1.15, 1.2))
    axis.legend()

    figure.savefig(output_file, format="svg")
    plt.close(figure)


def load_plot_data(input_json: Path) -> BenchmarkPlotData:
    payload = json.loads(input_json.read_text())
    ranked_cases = sorted(
        payload["cases"],
        key=lambda case: case["speedups"]["over_grep"],
        reverse=True,
    )

    return BenchmarkPlotData(
        title="Speedup of grep-rust over grep, ripgrep, and fastgrep",
        case_labels=[case["case_label"] for case in ranked_cases],
        series=[
            PlotSeries(
                label="vs grep",
                speedups=[case["speedups"]["over_grep"] for case in ranked_cases],
            ),
            PlotSeries(
                label="vs ripgrep",
                speedups=[
                    nan_or_value(case["speedups"]["over_ripgrep"])
                    for case in ranked_cases
                ],
            ),
            PlotSeries(
                label="vs fastgrep",
                speedups=[
                    nan_or_value(case["speedups"]["over_fastgrep"])
                    for case in ranked_cases
                ],
            ),
        ],
    )


def nan_or_value(value: float | None) -> float:
    return float("nan") if value is None else value


def render_plot_from_json(input_json: Path, output_file: Path) -> None:
    plot_data = load_plot_data(input_json)
    output_file.parent.mkdir(parents=True, exist_ok=True)
    write_svg_plot(
        output_file,
        plot_data.case_labels,
        plot_data.series,
        plot_data.title,
    )


def main() -> None:
    render_plot_from_json(DEFAULT_INPUT_JSON, DEFAULT_OUTPUT_FILE)


if __name__ == "__main__":
    main()

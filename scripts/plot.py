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

    return "\n".join(
        (" ".join(words[:best_split]), " ".join(words[best_split:]))
    )


def write_svg_plot(
    output_file: Path,
    case_labels: list[str],
    speedups: list[float],
    title: str,
) -> None:
    wrapped_labels = [wrap_case_label(label) for label in case_labels]
    default_width, default_height = plt.rcParams["figure.figsize"]
    longest_label_line = max(
        len(line) for label in wrapped_labels for line in label.splitlines()
    )
    figure_width = max(default_width, len(wrapped_labels) * 1.4, longest_label_line * 0.7)
    figure, axis = plt.subplots(figsize=(figure_width, default_height), layout="constrained")

    base_positions = list(range(len(wrapped_labels)))
    bar_colors = ["#2E8B57" if speedup >= 1.0 else "#C23B22" for speedup in speedups]
    bars = axis.bar(base_positions, speedups, color=bar_colors, width=0.8)

    axis.set_xticks(base_positions, labels=wrapped_labels)
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
            f"{speedup:.2f}",
            ha="center",
            va="bottom",
        )

    figure.savefig(output_file, format="svg")
    plt.close(figure)


def load_plot_data(input_json: Path) -> BenchmarkPlotData:
    payload = json.loads(input_json.read_text())
    ranked_cases = sorted(
        (
            (
                case["case_label"],
                case["series"]["grep baseline"]["mean_ms"]
                / case["series"]["grep-rust"]["mean_ms"],
            )
            for case in payload["cases"]
        ),
        key=lambda item: item[1],
        reverse=True,
    )
    return BenchmarkPlotData(
        title="Speedup of grep-rust over grep",
        case_labels=[label for label, _ in ranked_cases],
        speedups=[speedup for _, speedup in ranked_cases],
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

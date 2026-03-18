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
class PlotSeries:
    label: str
    means_ms: list[float]
    stddev_ms: list[float]


@dataclass(frozen=True)
class BenchmarkPlotData:
    title: str
    case_labels: list[str]
    series: list[PlotSeries]


def write_svg_plot(
    output_file: Path,
    case_labels: list[str],
    series: list[PlotSeries],
    title: str,
) -> None:
    default_width, default_height = plt.rcParams["figure.figsize"]
    longest_label_line = max(
        len(line) for label in case_labels for line in label.splitlines()
    )
    figure_width = max(default_width, len(case_labels) * 1.2, longest_label_line * 0.6)
    figure, axis = plt.subplots(figsize=(figure_width, default_height), layout="constrained")

    base_positions = list(range(len(case_labels)))
    bar_width = 0.8 / len(series)
    offsets = centered_offsets(len(series), bar_width)

    for offset, plot_series in zip(offsets, series):
        positions = [position + offset for position in base_positions]
        axis.bar(
            positions,
            plot_series.means_ms,
            yerr=plot_series.stddev_ms,
            width=bar_width,
            label=plot_series.label,
            color="#D34516" if plot_series.label == "grep-rust" else None,
        )

    axis.set_xticks(base_positions, labels=case_labels)
    plt.setp(axis.get_xticklabels(), rotation=0, ha="center", linespacing=0.95)
    axis.set_xlabel("Benchmark case")
    axis.set_ylabel("Mean runtime (ms)")
    axis.set_title(title)
    axis.set_axisbelow(True)
    axis.tick_params(axis="x", pad=4, length=0)
    axis.legend(frameon=False, loc="upper right")
    figure.savefig(output_file, format="svg")
    plt.close(figure)


def load_plot_data(input_json: Path) -> BenchmarkPlotData:
    payload = json.loads(input_json.read_text())
    case_labels = [case["case_label"] for case in payload["cases"]]
    series_labels = list(payload["cases"][0]["series"])
    series = [
        PlotSeries(
            label=series_label,
            means_ms=[
                case["series"][series_label]["mean_ms"] for case in payload["cases"]
            ],
            stddev_ms=[
                case["series"][series_label]["stddev_ms"] for case in payload["cases"]
            ],
        )
        for series_label in series_labels
    ]
    return BenchmarkPlotData(
        title=payload["title"],
        case_labels=case_labels,
        series=series,
    )


def render_plot_from_json(input_json: Path, output_file: Path) -> None:
    plot_data = load_plot_data(input_json)
    output_file.parent.mkdir(parents=True, exist_ok=True)
    write_svg_plot(
        output_file,
        plot_data.case_labels,
        plot_data.series,
        plot_data.title,
    )


def centered_offsets(series_count: int, bar_height: float) -> list[float]:
    midpoint = (series_count - 1) / 2
    return [(index - midpoint) * bar_height for index in range(series_count)]


def main() -> None:
    render_plot_from_json(DEFAULT_INPUT_JSON, DEFAULT_OUTPUT_FILE)


if __name__ == "__main__":
    main()

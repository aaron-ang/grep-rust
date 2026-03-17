from pathlib import Path
import math
import xml.etree.ElementTree as ET

SVG_NS = "http://www.w3.org/2000/svg"
FONT_FAMILY = "Helvetica, Arial, sans-serif"
BACKGROUND = "#f7f4ec"
AXIS = "#1f2933"
GRID = "#d8dee9"
LABEL = "#5b6c7d"
TEXT = "#243b53"
ANNOTATION = "#102a43"
TITLE = "#14202b"
BAR_COLORS = ("#c26a2d", "#3d6b99")


def as_svg_num(value: float | int) -> str:
    if isinstance(value, int):
        return str(value)
    return f"{value:.1f}"


def text_attrs(
    x: float | int,
    y: float | int,
    size: int,
    fill: str,
    anchor: str | None = None,
) -> dict[str, str]:
    attrs = {
        "x": as_svg_num(x),
        "y": as_svg_num(y),
        "font-size": as_svg_num(size),
        "font-family": FONT_FAMILY,
        "fill": fill,
    }
    if anchor is not None:
        attrs["text-anchor"] = anchor
    return attrs


def line_attrs(
    x1: float | int,
    y1: float | int,
    x2: float | int,
    y2: float | int,
    stroke: str,
    width: int,
) -> dict[str, str]:
    return {
        "x1": as_svg_num(x1),
        "y1": as_svg_num(y1),
        "x2": as_svg_num(x2),
        "y2": as_svg_num(y2),
        "stroke": stroke,
        "stroke-width": as_svg_num(width),
    }


def add(
    parent: ET.Element,
    tag: str,
    attrs: dict[str, str],
    text: str | None = None,
) -> ET.Element:
    element = ET.SubElement(parent, tag, attrs)
    if text is not None:
        element.text = text
    return element


def nice_tick_step(max_value: float, tick_count: int = 5) -> int:
    if max_value <= 0:
        return 1

    rough_step = max_value / tick_count
    magnitude = 10 ** math.floor(math.log10(rough_step))
    normalized = rough_step / magnitude

    if normalized <= 1:
        nice = 1
    elif normalized <= 2:
        nice = 2
    elif normalized <= 5:
        nice = 5
    else:
        nice = 10

    return int(nice * magnitude)


def write_svg_plot(
    output_file: Path,
    labels: list[str],
    means_ms: list[float],
    stddev_ms: list[float],
) -> None:
    ET.register_namespace("", SVG_NS)
    width = 1000
    height = 520
    margin_left = 90
    margin_right = 40
    margin_top = 60
    margin_bottom = 110
    chart_width = width - margin_left - margin_right
    chart_height = height - margin_top - margin_bottom
    max_value = max(mean + stddev for mean, stddev in zip(means_ms, stddev_ms))
    tick_step = nice_tick_step(max_value)
    tick_max = max(tick_step, math.ceil(max_value / tick_step) * tick_step)
    scale = chart_height / tick_max if tick_max > 0 else 1.0
    bar_width = min(240, chart_width / max(len(labels), 1) * 0.5)
    step = chart_width / max(len(labels), 1)
    x_axis_y = height - margin_bottom

    svg = ET.Element(
        "svg",
        {
            "xmlns": SVG_NS,
            "width": as_svg_num(width),
            "height": as_svg_num(height),
            "viewBox": f"0 0 {width} {height}",
        },
    )

    add(svg, "rect", {"width": "100%", "height": "100%", "fill": BACKGROUND})
    add(svg, "text", text_attrs(500, 30, 26, TITLE, "middle"), "grep Benchmark")
    add(
        svg,
        "line",
        line_attrs(margin_left, x_axis_y, width - margin_right, x_axis_y, AXIS, 2),
    )
    add(
        svg,
        "line",
        line_attrs(margin_left, margin_top, margin_left, x_axis_y, AXIS, 2),
    )

    for value in range(0, tick_max + tick_step, tick_step):
        y = x_axis_y - value * scale
        add(svg, "line", line_attrs(margin_left, y, width - margin_right, y, GRID, 1))
        add(
            svg,
            "text",
            text_attrs(margin_left - 12, y + 4, 12, LABEL, "end"),
            str(value),
        )

    for index, (label, mean, stddev) in enumerate(zip(labels, means_ms, stddev_ms)):
        center_x = margin_left + step * index + step / 2
        bar_height = mean * scale
        bar_x = center_x - bar_width / 2
        bar_y = x_axis_y - bar_height
        error_top = x_axis_y - (mean + stddev) * scale
        error_bottom = x_axis_y - max(mean - stddev, 0) * scale
        color = BAR_COLORS[index % len(BAR_COLORS)]

        add(
            svg,
            "rect",
            {
                "x": as_svg_num(bar_x),
                "y": as_svg_num(bar_y),
                "width": as_svg_num(bar_width),
                "height": as_svg_num(bar_height),
                "fill": color,
                "rx": "6",
            },
        )
        add(svg, "line", line_attrs(center_x, error_top, center_x, error_bottom, ANNOTATION, 2))
        add(svg, "line", line_attrs(center_x - 8, error_top, center_x + 8, error_top, ANNOTATION, 2))
        add(svg, "line", line_attrs(center_x - 8, error_bottom, center_x + 8, error_bottom, ANNOTATION, 2))
        add(
            svg,
            "text",
            text_attrs(center_x, x_axis_y + 28, 13, TEXT, "middle"),
            label,
        )

    add(svg, "text", text_attrs(20, margin_top - 20, 14, TEXT), "Mean time (ms)")
    add(svg, "text", text_attrs(width / 2, x_axis_y + 50, 14, TEXT, "middle"), "Benchmark target")
    ET.indent(svg)
    ET.ElementTree(svg).write(output_file, encoding="unicode", xml_declaration=True)

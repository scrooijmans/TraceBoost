#!/usr/bin/env python3
from __future__ import annotations

import argparse
import bisect
import json
import math
import pathlib
import statistics
import struct
from dataclasses import dataclass


@dataclass(frozen=True)
class HorizonEntry:
    horizon_id: str
    name: str
    vertical_domain: str
    vertical_unit: str
    values_file: pathlib.Path
    validity_file: pathlib.Path
    source_path: pathlib.Path


@dataclass(frozen=True)
class TransformEntry:
    transform_id: str
    sample_count: int
    inline_count: int
    xline_count: int
    sample_start_ms: float
    sample_step_ms: float
    depths_file: pathlib.Path
    validity_file: pathlib.Path
    grid_transform: dict


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Benchmark F3 synthetic horizon import fidelity and TWT<->depth conversion accuracy."
    )
    parser.add_argument("--store-path", type=pathlib.Path, required=True)
    parser.add_argument("--survey-package", type=pathlib.Path, required=True)
    parser.add_argument("--transform-id", type=str, default="velocity-functions-vint-survey-transform")
    parser.add_argument("--output-json", type=pathlib.Path)
    parser.add_argument("--output-md", type=pathlib.Path)
    return parser.parse_args()


def read_json(path: pathlib.Path) -> dict:
    return json.loads(path.read_text())


def read_f32le(path: pathlib.Path) -> list[float]:
    raw = path.read_bytes()
    if len(raw) % 4 != 0:
        raise SystemExit(f"expected f32le payload at {path}, found {len(raw)} bytes")
    return [value[0] for value in struct.iter_unpack("<f", raw)]


def read_u8(path: pathlib.Path) -> list[int]:
    return list(path.read_bytes())


def parse_horizons(store_path: pathlib.Path) -> dict[str, HorizonEntry]:
    manifest = read_json(store_path / "horizons" / "manifest.json")
    result: dict[str, HorizonEntry] = {}
    for item in manifest["horizons"]:
        result[item["id"]] = HorizonEntry(
            horizon_id=item["id"],
            name=item["name"],
            vertical_domain=item["vertical_domain"],
            vertical_unit=item["vertical_unit"],
            values_file=store_path / "horizons" / item["values_file"],
            validity_file=store_path / "horizons" / item["validity_file"],
            source_path=pathlib.Path(item["source_path"]),
        )
    return result


def parse_transform(store_path: pathlib.Path, transform_id: str) -> TransformEntry:
    manifest = read_json(store_path / "time-depth-transforms" / "manifest.json")
    for item in manifest["transforms"]:
        descriptor = item["descriptor"]
        if descriptor["id"] != transform_id:
            continue
        time_axis = descriptor["time_axis"]
        return TransformEntry(
            transform_id=descriptor["id"],
            sample_count=descriptor["sample_count"],
            inline_count=descriptor["inline_count"],
            xline_count=descriptor["xline_count"],
            sample_start_ms=float(time_axis["start"]),
            sample_step_ms=float(time_axis["step"]),
            depths_file=store_path / "time-depth-transforms" / item["depths_file"],
            validity_file=store_path / "time-depth-transforms" / item["validity_file"],
            grid_transform=descriptor["grid_transform"],
        )
    raise SystemExit(f"transform '{transform_id}' not found in {store_path / 'time-depth-transforms' / 'manifest.json'}")


def sample_axis_ms(transform: TransformEntry) -> list[float]:
    return [
        transform.sample_start_ms + transform.sample_step_ms * sample_index
        for sample_index in range(transform.sample_count)
    ]


def source_values_from_xyz(path: pathlib.Path) -> list[float]:
    values: list[float] = []
    with path.open("r", encoding="utf-8") as handle:
        for line in handle:
            stripped = line.strip()
            if not stripped or stripped.startswith("#") or stripped.startswith("//"):
                continue
            fields = stripped.replace(",", " ").replace(";", " ").split()
            if len(fields) < 3:
                raise SystemExit(f"invalid xyz row in {path}")
            values.append(float(fields[2]))
    return values


def parse_control_profile_cells(
    velocity_functions_path: pathlib.Path,
    grid_transform: dict,
    inline_count: int,
    xline_count: int,
) -> set[int]:
    origin_x = float(grid_transform["origin"]["x"])
    origin_y = float(grid_transform["origin"]["y"])
    inline_dx = float(grid_transform["inline_basis"]["x"])
    inline_dy = float(grid_transform["inline_basis"]["y"])
    xline_dx = float(grid_transform["xline_basis"]["x"])
    xline_dy = float(grid_transform["xline_basis"]["y"])
    determinant = inline_dx * xline_dy - inline_dy * xline_dx
    if abs(determinant) <= 1e-12:
        raise SystemExit("grid transform is singular")

    control_cells: set[int] = set()
    seen_xy: set[tuple[float, float]] = set()
    with velocity_functions_path.open("r", encoding="utf-8") as handle:
        for line in handle:
            stripped = line.strip()
            if not stripped or stripped.startswith("CDP-X"):
                continue
            fields = stripped.split()
            if len(fields) < 7:
                continue
            x = round(float(fields[0]), 6)
            y = round(float(fields[1]), 6)
            xy = (x, y)
            if xy in seen_xy:
                continue
            seen_xy.add(xy)

            dx = x - origin_x
            dy = y - origin_y
            inline = (dx * xline_dy - dy * xline_dx) / determinant
            xline = (dy * inline_dx - dx * inline_dy) / determinant
            inline_index = int(round(inline))
            xline_index = int(round(xline))
            if not (0 <= inline_index < inline_count and 0 <= xline_index < xline_count):
                raise SystemExit(f"control profile ({x}, {y}) mapped outside the survey grid")
            if abs(inline - inline_index) > 1e-3 or abs(xline - xline_index) > 1e-3:
                raise SystemExit(
                    f"control profile ({x}, {y}) did not align to an integer cell: inline={inline}, xline={xline}"
                )
            control_cells.add(inline_index * xline_count + xline_index)
    return control_cells


def depth_at_time(depths_m: list[float], times_ms: list[float], target_time_ms: float) -> float:
    if target_time_ms <= times_ms[0]:
        return depths_m[0]
    if target_time_ms >= times_ms[-1]:
        return depths_m[-1]
    upper_index = bisect.bisect_left(times_ms, target_time_ms)
    lower_index = upper_index - 1
    lower_time = times_ms[lower_index]
    upper_time = times_ms[upper_index]
    if abs(upper_time - lower_time) <= 1e-12:
        return depths_m[upper_index]
    t = max(0.0, min(1.0, (target_time_ms - lower_time) / (upper_time - lower_time)))
    return depths_m[lower_index] + (depths_m[upper_index] - depths_m[lower_index]) * t


def time_at_depth(depths_m: list[float], times_ms: list[float], target_depth_m: float) -> float:
    if target_depth_m <= depths_m[0]:
        return times_ms[0]
    if target_depth_m >= depths_m[-1]:
        return times_ms[-1]
    upper_index = bisect.bisect_left(depths_m, target_depth_m)
    lower_index = upper_index - 1
    lower_depth = depths_m[lower_index]
    upper_depth = depths_m[upper_index]
    if abs(upper_depth - lower_depth) <= 1e-12:
        return times_ms[upper_index]
    t = max(0.0, min(1.0, (target_depth_m - lower_depth) / (upper_depth - lower_depth)))
    return times_ms[lower_index] + (times_ms[upper_index] - times_ms[lower_index]) * t


def metrics(errors: list[float]) -> dict[str, float | int]:
    if not errors:
        return {
            "count": 0,
            "rmse": math.nan,
            "mae": math.nan,
            "mean_bias": math.nan,
            "p95_abs": math.nan,
            "max_abs": math.nan,
        }
    abs_errors = sorted(abs(value) for value in errors)
    p95_index = min(len(abs_errors) - 1, max(0, math.ceil(0.95 * len(abs_errors)) - 1))
    squared = sum(value * value for value in errors)
    mae = sum(abs(value) for value in errors) / len(errors)
    return {
        "count": len(errors),
        "rmse": math.sqrt(squared / len(errors)),
        "mae": mae,
        "mean_bias": statistics.fmean(errors),
        "p95_abs": abs_errors[p95_index],
        "max_abs": abs_errors[-1],
    }


def benchmark_import_fidelity(
    horizon: HorizonEntry,
    expected_cells: int,
    control_cells: set[int],
) -> dict[str, object]:
    if not horizon.source_path.exists():
        return {
            "horizon_id": horizon.horizon_id,
            "domain": horizon.vertical_domain,
            "unit": horizon.vertical_unit,
            "skipped": True,
            "skip_reason": f"source path '{horizon.source_path}' is not a local ASCII horizon file",
        }
    source_values = source_values_from_xyz(horizon.source_path)
    if len(source_values) != expected_cells:
        raise SystemExit(
            f"source horizon {horizon.source_path} has {len(source_values)} rows, expected {expected_cells}"
        )
    imported_values = read_f32le(horizon.values_file)
    imported_validity = read_u8(horizon.validity_file)

    full_errors: list[float] = []
    control_errors: list[float] = []
    invalid_count = 0
    for offset, expected in enumerate(source_values):
        if imported_validity[offset] == 0:
            invalid_count += 1
            continue
        error = imported_values[offset] - expected
        full_errors.append(error)
        if offset in control_cells:
            control_errors.append(error)
    return {
        "horizon_id": horizon.horizon_id,
        "domain": horizon.vertical_domain,
        "unit": horizon.vertical_unit,
        "invalid_cell_count": invalid_count,
        "full_grid": metrics(full_errors),
        "control_profiles": metrics(control_errors),
    }


def benchmark_conversion_pair(
    time_horizon: HorizonEntry,
    depth_horizon: HorizonEntry,
    transform: TransformEntry,
    control_cells: set[int],
) -> dict[str, object]:
    time_values = read_f32le(time_horizon.values_file)
    time_validity = read_u8(time_horizon.validity_file)
    depth_values = read_f32le(depth_horizon.values_file)
    depth_validity = read_u8(depth_horizon.validity_file)
    transform_depths = read_f32le(transform.depths_file)
    transform_validity = read_u8(transform.validity_file)
    times_ms = sample_axis_ms(transform)
    samples_per_trace = transform.sample_count

    full_twt_to_depth_errors: list[float] = []
    control_twt_to_depth_errors: list[float] = []
    full_depth_to_twt_errors: list[float] = []
    control_depth_to_twt_errors: list[float] = []

    cell_count = transform.inline_count * transform.xline_count
    for cell_index in range(cell_count):
        trace_start = cell_index * samples_per_trace
        trace_end = trace_start + samples_per_trace
        trace_valid = any(value != 0 for value in transform_validity[trace_start:trace_end])
        if not trace_valid:
            continue
        trace_depths = transform_depths[trace_start:trace_end]

        if time_validity[cell_index] != 0 and depth_validity[cell_index] != 0:
            converted_depth = depth_at_time(trace_depths, times_ms, time_values[cell_index])
            depth_error = converted_depth - depth_values[cell_index]
            full_twt_to_depth_errors.append(depth_error)
            if cell_index in control_cells:
                control_twt_to_depth_errors.append(depth_error)

            converted_time = time_at_depth(trace_depths, times_ms, depth_values[cell_index])
            time_error = converted_time - time_values[cell_index]
            full_depth_to_twt_errors.append(time_error)
            if cell_index in control_cells:
                control_depth_to_twt_errors.append(time_error)

    return {
        "horizon_pair": time_horizon.horizon_id.replace("_twt_ms", ""),
        "time_horizon_id": time_horizon.horizon_id,
        "depth_horizon_id": depth_horizon.horizon_id,
        "twt_to_depth_m": {
            "full_grid": metrics(full_twt_to_depth_errors),
            "control_profiles": metrics(control_twt_to_depth_errors),
        },
        "depth_to_twt_ms": {
            "full_grid": metrics(full_depth_to_twt_errors),
            "control_profiles": metrics(control_depth_to_twt_errors),
        },
    }


def paired_horizons(horizons: dict[str, HorizonEntry]) -> list[tuple[HorizonEntry, HorizonEntry]]:
    pairs: list[tuple[HorizonEntry, HorizonEntry]] = []
    for index in range(1, 100):
        prefix = f"horizon_{index:02d}"
        time_id = f"{prefix}_twt_ms"
        depth_id = f"{prefix}_depth_m"
        if time_id not in horizons or depth_id not in horizons:
            if index == 1:
                continue
            break
        pairs.append((horizons[time_id], horizons[depth_id]))
    if not pairs:
        raise SystemExit("no TWT/depth horizon pairs were found in the store")
    return pairs


def summarize_conversion(conversion_pairs: list[dict[str, object]]) -> dict[str, object]:
    def gather(metric_group: str, scope: str, field: str) -> list[float]:
        values: list[float] = []
        for pair in conversion_pairs:
            value = pair[metric_group][scope][field]
            if not math.isnan(value):
                values.append(value)
        return values

    summary: dict[str, object] = {}
    for metric_group in ("twt_to_depth_m", "depth_to_twt_ms"):
        summary[metric_group] = {}
        for scope in ("full_grid", "control_profiles"):
            summary[metric_group][scope] = {
                "mean_rmse": statistics.fmean(gather(metric_group, scope, "rmse")),
                "mean_mae": statistics.fmean(gather(metric_group, scope, "mae")),
                "worst_p95_abs": max(gather(metric_group, scope, "p95_abs")),
                "worst_max_abs": max(gather(metric_group, scope, "max_abs")),
            }
    return summary


def markdown_report(report: dict[str, object]) -> str:
    lines = [
        "# F3 Synthetic Horizon Conversion Benchmark",
        "",
        f"- store: `{report['store_path']}`",
        f"- survey package: `{report['survey_package']}`",
        f"- transform: `{report['transform_id']}`",
        f"- control profiles: `{report['control_profile_count']}`",
        "",
        "## Summary",
    ]
    summary = report["conversion_summary"]
    for metric_group, unit in (("twt_to_depth_m", "m"), ("depth_to_twt_ms", "ms")):
        for scope in ("full_grid", "control_profiles"):
            stats = summary[metric_group][scope]
            lines.append(
                f"- {metric_group} / {scope}: mean RMSE {stats['mean_rmse']:.6f} {unit}, "
                f"mean MAE {stats['mean_mae']:.6f} {unit}, "
                f"worst P95 {stats['worst_p95_abs']:.6f} {unit}, "
                f"worst max {stats['worst_max_abs']:.6f} {unit}"
            )
    lines.extend(["", "## Notes"])
    transform_id = str(report["transform_id"]).lower()
    if "paired-horizon" in transform_id:
        lines.append("- This benchmark reflects the paired-horizon transform path, so the remaining error is dominated by ASCII quantization and piecewise-linear resampling against the stored survey sample axis.")
        lines.append("- Control-profile statistics are reported for the authored velocity-function locations, but they are only a reporting subset for this paired-horizon transform.")
    else:
        lines.append("- Full-grid error is dominated by lateral interpolation choice in the sparse Vint model.")
        lines.append("- Control-profile error isolates behavior at authored velocity-function locations.")
    lines.append("- Import fidelity checks compare stored canonical horizon grids against the original ASCII source rows.")
    return "\n".join(lines) + "\n"


def main() -> None:
    args = parse_args()
    store_path = args.store_path.resolve()
    survey_package = args.survey_package.resolve()

    horizons = parse_horizons(store_path)
    transform = parse_transform(store_path, args.transform_id)
    expected_cells = transform.inline_count * transform.xline_count
    control_cells = parse_control_profile_cells(
        survey_package / "Velocity_functions.txt",
        transform.grid_transform,
        transform.inline_count,
        transform.xline_count,
    )

    import_fidelity = [
        benchmark_import_fidelity(horizon, expected_cells, control_cells)
        for horizon in sorted(horizons.values(), key=lambda entry: entry.horizon_id)
    ]
    conversion_pairs = [
        benchmark_conversion_pair(time_horizon, depth_horizon, transform, control_cells)
        for time_horizon, depth_horizon in paired_horizons(horizons)
    ]

    report = {
        "store_path": str(store_path),
        "survey_package": str(survey_package),
        "transform_id": transform.transform_id,
        "inline_count": transform.inline_count,
        "xline_count": transform.xline_count,
        "sample_count": transform.sample_count,
        "control_profile_count": len(control_cells),
        "import_fidelity": import_fidelity,
        "conversion_pairs": conversion_pairs,
        "conversion_summary": summarize_conversion(conversion_pairs),
        "reference_notes": {
            "reviewed_algorithms": [
                "OpendTect time-depth converter classes",
                "Madagascar time-to-depth conversion references",
            ],
            "external_api_benchmark_status": "not_executed",
        },
    }

    payload = json.dumps(report, indent=2) + "\n"
    if args.output_json:
        args.output_json.parent.mkdir(parents=True, exist_ok=True)
        args.output_json.write_text(payload, encoding="utf-8")
    else:
        print(payload)

    if args.output_md:
        args.output_md.parent.mkdir(parents=True, exist_ok=True)
        args.output_md.write_text(markdown_report(report), encoding="utf-8")


if __name__ == "__main__":
    main()

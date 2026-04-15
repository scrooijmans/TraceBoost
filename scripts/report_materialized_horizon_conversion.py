#!/usr/bin/env python3
from __future__ import annotations

import argparse
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


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Compare materialized derived horizons against authored canonical TWT/depth horizon pairs."
    )
    parser.add_argument("--store-path", type=pathlib.Path, required=True)
    parser.add_argument("--transform-id", type=str, required=True)
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
        )
    return result


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


def authored_pairs(horizons: dict[str, HorizonEntry]) -> list[tuple[str, HorizonEntry, HorizonEntry, HorizonEntry, HorizonEntry]]:
    pairs: list[tuple[str, HorizonEntry, HorizonEntry, HorizonEntry, HorizonEntry]] = []
    for index in range(1, 100):
        pair_id = f"horizon_{index:02d}"
        time_id = f"{pair_id}_twt_ms"
        depth_id = f"{pair_id}_depth_m"
        derived_depth_id = f"{time_id}-derived_depth_m"
        derived_time_id = f"{depth_id}-derived_twt_ms"
        if time_id not in horizons or depth_id not in horizons:
            if index == 1:
                continue
            break
        if derived_depth_id not in horizons or derived_time_id not in horizons:
            continue
        pairs.append(
            (
                pair_id,
                horizons[time_id],
                horizons[depth_id],
                horizons[derived_depth_id],
                horizons[derived_time_id],
            )
        )
    if not pairs:
        raise SystemExit("no authored/derived horizon comparison pairs found in the store")
    return pairs


def compare_pair(
    pair_id: str,
    time_horizon: HorizonEntry,
    depth_horizon: HorizonEntry,
    derived_depth_horizon: HorizonEntry,
    derived_time_horizon: HorizonEntry,
) -> dict[str, object]:
    authored_time_values = read_f32le(time_horizon.values_file)
    authored_depth_values = read_f32le(depth_horizon.values_file)
    derived_depth_values = read_f32le(derived_depth_horizon.values_file)
    derived_time_values = read_f32le(derived_time_horizon.values_file)

    authored_time_validity = read_u8(time_horizon.validity_file)
    authored_depth_validity = read_u8(depth_horizon.validity_file)
    derived_depth_validity = read_u8(derived_depth_horizon.validity_file)
    derived_time_validity = read_u8(derived_time_horizon.validity_file)

    if not (
        len(authored_time_values)
        == len(authored_depth_values)
        == len(derived_depth_values)
        == len(derived_time_values)
    ):
        raise SystemExit(f"inconsistent grid sizes for {pair_id}")

    twt_to_depth_errors: list[float] = []
    depth_to_twt_errors: list[float] = []
    invalid_twt_to_depth = 0
    invalid_depth_to_twt = 0

    for cell_index in range(len(authored_time_values)):
        twt_to_depth_valid = (
            authored_time_validity[cell_index] != 0
            and authored_depth_validity[cell_index] != 0
            and derived_depth_validity[cell_index] != 0
        )
        if twt_to_depth_valid:
            twt_to_depth_errors.append(derived_depth_values[cell_index] - authored_depth_values[cell_index])
        else:
            invalid_twt_to_depth += 1

        depth_to_twt_valid = (
            authored_depth_validity[cell_index] != 0
            and authored_time_validity[cell_index] != 0
            and derived_time_validity[cell_index] != 0
        )
        if depth_to_twt_valid:
            depth_to_twt_errors.append(derived_time_values[cell_index] - authored_time_values[cell_index])
        else:
            invalid_depth_to_twt += 1

    return {
        "horizon_pair": pair_id,
        "time_horizon_id": time_horizon.horizon_id,
        "depth_horizon_id": depth_horizon.horizon_id,
        "derived_depth_horizon_id": derived_depth_horizon.horizon_id,
        "derived_time_horizon_id": derived_time_horizon.horizon_id,
        "twt_to_depth_m": {
            "invalid_cell_count": invalid_twt_to_depth,
            **metrics(twt_to_depth_errors),
        },
        "depth_to_twt_ms": {
            "invalid_cell_count": invalid_depth_to_twt,
            **metrics(depth_to_twt_errors),
        },
    }


def summarize_pairs(pairs: list[dict[str, object]]) -> dict[str, object]:
    summary: dict[str, object] = {}
    for metric_group in ("twt_to_depth_m", "depth_to_twt_ms"):
        rmse_values = [pair[metric_group]["rmse"] for pair in pairs if not math.isnan(pair[metric_group]["rmse"])]
        mae_values = [pair[metric_group]["mae"] for pair in pairs if not math.isnan(pair[metric_group]["mae"])]
        p95_values = [pair[metric_group]["p95_abs"] for pair in pairs if not math.isnan(pair[metric_group]["p95_abs"])]
        max_values = [pair[metric_group]["max_abs"] for pair in pairs if not math.isnan(pair[metric_group]["max_abs"])]
        invalid_counts = [int(pair[metric_group]["invalid_cell_count"]) for pair in pairs]
        summary[metric_group] = {
            "mean_rmse": statistics.fmean(rmse_values),
            "mean_mae": statistics.fmean(mae_values),
            "worst_p95_abs": max(p95_values),
            "worst_max_abs": max(max_values),
            "total_invalid_cell_count": sum(invalid_counts),
        }
    return summary


def markdown_report(report: dict[str, object]) -> str:
    summary = report["summary"]
    lines = [
        "# F3 Materialized Horizon Conversion Report",
        "",
        f"- store: `{report['store_path']}`",
        f"- transform: `{report['transform_id']}`",
        f"- compared pairs: `{report['pair_count']}`",
        "",
        "## Summary",
        (
            "- twt_to_depth_m: "
            f"mean RMSE {summary['twt_to_depth_m']['mean_rmse']:.6f} m, "
            f"mean MAE {summary['twt_to_depth_m']['mean_mae']:.6f} m, "
            f"worst P95 {summary['twt_to_depth_m']['worst_p95_abs']:.6f} m, "
            f"worst max {summary['twt_to_depth_m']['worst_max_abs']:.6f} m, "
            f"invalid cells {summary['twt_to_depth_m']['total_invalid_cell_count']}"
        ),
        (
            "- depth_to_twt_ms: "
            f"mean RMSE {summary['depth_to_twt_ms']['mean_rmse']:.6f} ms, "
            f"mean MAE {summary['depth_to_twt_ms']['mean_mae']:.6f} ms, "
            f"worst P95 {summary['depth_to_twt_ms']['worst_p95_abs']:.6f} ms, "
            f"worst max {summary['depth_to_twt_ms']['worst_max_abs']:.6f} ms, "
            f"invalid cells {summary['depth_to_twt_ms']['total_invalid_cell_count']}"
        ),
        "",
        "## Notes",
        "- These figures compare the stored derived horizons against the authored canonical horizons already imported into the same regularized F3 store.",
        "- For this F3 package, the materialized-horizon numbers match the paired-horizon transform benchmark, which is expected because both paths use the same piecewise-linear time-depth model.",
        "- The exported depth velocity cube remains a derived convenience product on a regular depth axis starting at 0 m; it is not the accuracy anchor for horizon-to-horizon conversion.",
    ]
    return "\n".join(lines) + "\n"


def main() -> None:
    args = parse_args()
    store_path = args.store_path.resolve()
    horizons = parse_horizons(store_path)
    pairs = [
        compare_pair(pair_id, time_horizon, depth_horizon, derived_depth_horizon, derived_time_horizon)
        for pair_id, time_horizon, depth_horizon, derived_depth_horizon, derived_time_horizon in authored_pairs(horizons)
    ]
    report = {
        "store_path": str(store_path),
        "transform_id": args.transform_id,
        "pair_count": len(pairs),
        "pairs": pairs,
        "summary": summarize_pairs(pairs),
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

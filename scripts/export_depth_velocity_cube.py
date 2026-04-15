#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import math
import pathlib
import sys

import numpy as np


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Export a depth-domain interval-velocity cube from a stored survey time-depth transform."
    )
    parser.add_argument("--store-path", type=pathlib.Path, required=True)
    parser.add_argument("--transform-id", type=str, required=True)
    parser.add_argument("--output-dir", type=pathlib.Path, required=True)
    parser.add_argument("--depth-step-m", type=float, default=5.0)
    parser.add_argument("--depth-margin-m", type=float, default=25.0)
    return parser.parse_args()


def read_json(path: pathlib.Path) -> dict:
    return json.loads(path.read_text())


def parse_transform(store_path: pathlib.Path, transform_id: str) -> dict:
    manifest = read_json(store_path / "time-depth-transforms" / "manifest.json")
    for item in manifest["transforms"]:
        descriptor = item["descriptor"]
        if descriptor["id"] != transform_id:
            continue
        descriptor = dict(descriptor)
        descriptor["depths_file"] = str(store_path / "time-depth-transforms" / item["depths_file"])
        descriptor["validity_file"] = str(store_path / "time-depth-transforms" / item["validity_file"])
        return descriptor
    raise SystemExit(f"transform '{transform_id}' not found in {store_path / 'time-depth-transforms' / 'manifest.json'}")


def sample_axis_ms(transform: dict) -> np.ndarray:
    axis = transform["time_axis"]
    return np.asarray(
        [
        float(axis["start"]) + float(axis["step"]) * sample_index
        for sample_index in range(int(axis["count"]))
        ],
        dtype=np.float32,
    )


def main() -> None:
    args = parse_args()
    store_path = args.store_path.resolve()
    output_dir = args.output_dir.resolve()
    output_dir.mkdir(parents=True, exist_ok=True)

    if args.depth_step_m <= 0.0:
        raise SystemExit("--depth-step-m must be positive")
    if args.depth_margin_m < 0.0:
        raise SystemExit("--depth-margin-m must be non-negative")

    transform = parse_transform(store_path, args.transform_id)
    depths_file = pathlib.Path(transform["depths_file"])
    validity_file = pathlib.Path(transform["validity_file"])
    transform_depths = np.fromfile(depths_file, dtype="<f4")
    transform_validity = np.fromfile(validity_file, dtype=np.uint8)

    inline_count = int(transform["inline_count"])
    xline_count = int(transform["xline_count"])
    time_sample_count = int(transform["sample_count"])
    trace_count = inline_count * xline_count
    expected_cells = trace_count * time_sample_count
    if transform_depths.size != expected_cells or transform_validity.size != expected_cells:
        raise SystemExit("transform payload size does not match its declared shape")

    times_ms = sample_axis_ms(transform)
    transform_depths = transform_depths.reshape(trace_count, time_sample_count)
    transform_validity = transform_validity.reshape(trace_count, time_sample_count)
    valid_traces = np.all(transform_validity != 0, axis=1)
    if not np.any(valid_traces):
        raise SystemExit("transform does not contain any fully valid traces")

    max_depth_m = float(np.max(transform_depths[valid_traces, -1]))
    depth_extent_m = max_depth_m + args.depth_margin_m
    depth_sample_count = int(math.ceil(depth_extent_m / args.depth_step_m)) + 1
    depth_axis_m = np.arange(depth_sample_count, dtype=np.float32) * np.float32(args.depth_step_m)

    values = np.zeros((trace_count, depth_sample_count), dtype=np.float32)
    validity = np.zeros((trace_count, depth_sample_count), dtype=np.uint8)
    invalid_trace_count = int((~valid_traces).sum())
    delta_times_ms = np.diff(times_ms)
    if np.any(delta_times_ms <= 0.0):
        raise SystemExit("transform time axis must increase strictly")

    batch_size = 512
    for batch_start in range(0, trace_count, batch_size):
        batch_end = min(trace_count, batch_start + batch_size)
        batch_valid_mask = valid_traces[batch_start:batch_end]
        if not np.any(batch_valid_mask):
            continue
        batch_trace_indices = np.nonzero(batch_valid_mask)[0] + batch_start
        batch_depths = transform_depths[batch_trace_indices]
        batch_delta_depths = np.diff(batch_depths, axis=1)
        if np.any(batch_delta_depths < 0.0):
            raise SystemExit("transform trace is not monotone in depth")
        segment_velocities = 2.0 * batch_delta_depths / (delta_times_ms[None, :] * 0.001)
        segment_indices = np.sum(
            batch_depths[:, :, None] < depth_axis_m[None, None, :],
            axis=1,
            dtype=np.int16,
        )
        segment_indices = np.clip(segment_indices - 1, 0, segment_velocities.shape[1] - 1)
        batch_values = np.take_along_axis(segment_velocities, segment_indices, axis=1)
        values[batch_trace_indices] = batch_values
        validity[batch_trace_indices] = 1

    manifest = {
        "version": 1,
        "source_store_path": str(store_path),
        "source_transform_id": transform["id"],
        "value_name": "interval_velocity",
        "value_unit": "m/s",
        "inline_count": inline_count,
        "xline_count": xline_count,
        "sample_count": depth_sample_count,
        "depth_axis": {
            "domain": "depth",
            "unit": "m",
            "start": 0.0,
            "step": args.depth_step_m,
            "count": depth_sample_count,
        },
        "max_depth_m": max_depth_m,
        "depth_extent_m": depth_extent_m,
        "coordinate_reference": transform.get("coordinate_reference"),
        "grid_transform": transform.get("grid_transform"),
        "invalid_trace_count": invalid_trace_count,
        "values_file": "interval_velocity.values.f32le.bin",
        "validity_file": "interval_velocity.validity.u8.bin",
        "notes": [
            "Depth-domain interval-velocity cube derived from the stored survey time-depth transform.",
            "The depth axis starts at 0 m and extends beyond the deepest transform sample by the requested margin.",
            "Per-depth-sample interval velocity is assigned from the containing time-depth segment.",
        ],
    }

    if sys.byteorder != "little":
        values.byteswap(inplace=True)
    values.tofile(output_dir / manifest["values_file"])
    validity.tofile(output_dir / manifest["validity_file"])
    (output_dir / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")

    print(
        json.dumps(
            {
                "output_dir": str(output_dir),
                "source_transform_id": transform["id"],
                "inline_count": inline_count,
                "xline_count": xline_count,
                "sample_count": depth_sample_count,
                "depth_step_m": args.depth_step_m,
                "depth_extent_m": depth_extent_m,
                "max_depth_m": max_depth_m,
                "invalid_trace_count": invalid_trace_count,
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    main()

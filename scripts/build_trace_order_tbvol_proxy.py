#!/usr/bin/env python
"""Build a benchmark-only tbvol proxy from SEG-Y trace order.

This is intentionally not a geometry solution. It exists to create a dense,
exact f32 volume for compression benchmarking when a SEG-Y file lacks usable
inline/xline headers but does carry a stable repeating trace-number cycle.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

import numpy as np
import segyio


def detect_trace_grid(trace_numbers: np.ndarray, tracecount: int) -> tuple[int, int]:
    if trace_numbers.size == 0:
        raise ValueError("SEG-Y contains no traces")

    traces_per_line = int(trace_numbers.max())
    if traces_per_line <= 0:
        raise ValueError("TraceNumber header does not provide a usable cycle")
    if tracecount % traces_per_line != 0:
        raise ValueError(
            f"tracecount {tracecount} is not divisible by detected traces-per-line {traces_per_line}"
        )

    expected = np.tile(np.arange(1, traces_per_line + 1, dtype=trace_numbers.dtype), tracecount // traces_per_line)
    if not np.array_equal(trace_numbers, expected):
        raise ValueError(
            "TraceNumber header does not form a clean repeating 1..N cycle; "
            "ordered proxy would be unreliable"
        )

    return tracecount // traces_per_line, traces_per_line


def build_proxy(input_path: Path, output_dir: Path) -> dict:
    with segyio.open(str(input_path), strict=False, ignore_geometry=True) as handle:
        tracecount = handle.tracecount
        samples = len(handle.samples)
        trace_numbers = np.asarray(handle.attributes(segyio.TraceField.TraceNumber)[:], dtype=np.int32)
        line_count, traces_per_line = detect_trace_grid(trace_numbers, tracecount)

        data = np.empty((line_count, traces_per_line, samples), dtype=np.float32)
        for line_index in range(line_count):
            start = line_index * traces_per_line
            stop = start + traces_per_line
            data[line_index, :, :] = np.asarray(handle.trace.raw[start:stop], dtype=np.float32)

    output_dir.mkdir(parents=True, exist_ok=True)
    amplitude_path = output_dir / "amplitude.bin"
    manifest_path = output_dir / "manifest.json"
    data.tofile(amplitude_path)

    manifest = {
        "format": "tbvol",
        "version": 1,
        "volume": {
            "kind": "BenchmarkProxy",
            "source": {"source_path": str(input_path)},
            "shape": [line_count, traces_per_line, samples],
        },
        "tile_shape": [line_count, traces_per_line, samples],
        "tile_grid_shape": [1, 1],
        "sample_type": "f32",
        "endianness": "little",
        "has_occupancy": False,
        "amplitude_tile_bytes": int(data.nbytes),
        "occupancy_tile_bytes": None,
    }
    manifest_path.write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    return {
        "source_path": str(input_path),
        "output_dir": str(output_dir),
        "shape": [line_count, traces_per_line, samples],
        "tracecount": tracecount,
        "traces_per_line": traces_per_line,
    }


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Build a dense trace-order tbvol proxy from a SEG-Y file for compression benchmarking."
    )
    parser.add_argument("input", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()

    summary = build_proxy(args.input, args.output)
    print(json.dumps(summary, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

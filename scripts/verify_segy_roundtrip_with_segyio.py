from __future__ import annotations

import argparse
import json
from pathlib import Path

import numpy as np
import segyio


def load_signature(path: Path) -> dict:
    with segyio.open(str(path), ignore_geometry=True) as handle:
        text_headers = [
            bytes(handle.text[index]) for index in range(1 + int(handle.ext_headers))
        ]
        binary_header = {int(key): int(value) for key, value in dict(handle.bin).items()}
        trace_headers = [
            {int(key): int(value) for key, value in dict(handle.header[index]).items()}
            for index in range(handle.tracecount)
        ]
        traces = np.asarray(handle.trace.raw[:])
        return {
            "path": str(path),
            "tracecount": int(handle.tracecount),
            "samples": int(len(handle.samples)),
            "format": int(handle.format),
            "ext_headers": int(handle.ext_headers),
            "text_headers": text_headers,
            "binary_header": binary_header,
            "trace_headers": trace_headers,
            "traces": traces,
        }


def compare(original: dict, candidate: dict) -> dict:
    binary_match = original["binary_header"] == candidate["binary_header"]
    trace_header_match = original["trace_headers"] == candidate["trace_headers"]
    text_header_match = original["text_headers"] == candidate["text_headers"]
    amplitude_match = np.array_equal(original["traces"], candidate["traces"])
    amplitude_max_abs_diff = float(
        np.max(np.abs(original["traces"].astype(np.float64) - candidate["traces"].astype(np.float64)))
    )
    return {
        "tracecount_match": original["tracecount"] == candidate["tracecount"],
        "samples_match": original["samples"] == candidate["samples"],
        "format_match": original["format"] == candidate["format"],
        "ext_headers_match": original["ext_headers"] == candidate["ext_headers"],
        "text_headers_match": text_header_match,
        "binary_header_match": binary_match,
        "trace_headers_match": trace_header_match,
        "amplitude_match": amplitude_match,
        "amplitude_max_abs_diff": amplitude_max_abs_diff,
        "original": {
            "tracecount": original["tracecount"],
            "samples": original["samples"],
            "format": original["format"],
            "ext_headers": original["ext_headers"],
        },
        "candidate": {
            "tracecount": candidate["tracecount"],
            "samples": candidate["samples"],
            "format": candidate["format"],
            "ext_headers": candidate["ext_headers"],
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Compare two SEG-Y files with segyio and report roundtrip-safe invariants."
    )
    parser.add_argument("original", type=Path)
    parser.add_argument("candidate", type=Path)
    args = parser.parse_args()

    original = load_signature(args.original)
    candidate = load_signature(args.candidate)
    result = compare(original, candidate)
    print(json.dumps(result, indent=2, sort_keys=True))

    ok = (
        result["tracecount_match"]
        and result["samples_match"]
        and result["format_match"]
        and result["ext_headers_match"]
        and result["text_headers_match"]
        and result["binary_header_match"]
        and result["trace_headers_match"]
        and result["amplitude_match"]
    )
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())

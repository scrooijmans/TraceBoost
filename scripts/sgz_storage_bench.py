#!/usr/bin/env python
"""External SGZ benchmark runner for TraceBoost storage evaluation.

This script mirrors the role of ``openvds_storage_bench.cpp`` for the
``seismic-zfp`` / SGZ format. It is intentionally external to the shared Rust
runtime because SGZ is currently being evaluated as a comparison target rather
than adopted as an in-runtime backend.
"""

from __future__ import annotations

import argparse
import importlib
import json
import math
import shutil
import subprocess
import sys
import time
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any, Callable, Iterable

import numpy as np


DEFAULT_TRACEBOOST_REPO = Path(__file__).resolve().parents[1]
DEFAULT_SEISMIC_ZFP_REPO = DEFAULT_TRACEBOOST_REPO.parent / "seismic-zfp"
RUNTIME_BIN_RELATIVE = Path("target") / "release" / "seis-runtime.exe"


@dataclass
class EnvCheckResult:
    python_version: str
    python_executable: str
    seismic_zfp_repo: str
    seismic_zfp_repo_exists: bool
    cargo_found: bool
    modules: dict[str, str]


@dataclass
class BenchmarkResult:
    dataset_name: str
    shape: list[int]
    bits_per_voxel: float
    blockshape: list[int]
    sample_interval_ms: float
    scalar_factor: float
    phase_rotation_degrees: float
    bandpass_hz: list[float]
    sgz_creation_ms: float
    input_store_bytes: int
    input_file_count: int
    inline_section_read_ms: float
    xline_section_read_ms: float
    preview_amplitude_scalar_ms: float
    preview_trace_rms_normalize_ms: float
    preview_phase_rotation_ms: float
    preview_bandpass_ms: float
    preview_bandpass_phase_rotation_ms: float
    preview_pipeline_ms: float
    apply_amplitude_scalar_ms: float
    apply_trace_rms_normalize_ms: float
    apply_phase_rotation_ms: float
    apply_bandpass_ms: float
    apply_bandpass_phase_rotation_ms: float
    apply_pipeline_ms: float
    pipeline_output_bytes: int
    pipeline_output_file_count: int
    decode_volume_mean_abs_error: float
    decode_volume_max_abs_error: float
    sgz_to_segy_ms: float | None
    segy_to_tbvol_ms: float | None
    sgz_to_tbvol_total_ms: float | None
    tbvol_bytes: int | None
    tbvol_file_count: int | None


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()

    if args.command == "check-env":
        result = check_environment(args.seismic_zfp_repo)
        print(json.dumps(asdict(result), indent=2))
        return 0

    try:
        if args.command == "benchmark-synthetic":
            result = benchmark_synthetic(args)
        else:
            parser.error(f"unsupported command: {args.command}")
            return 2
    except BenchmarkError as error:
        print(error, file=sys.stderr)
        return 2
    except KeyboardInterrupt:
        print("benchmark cancelled", file=sys.stderr)
        return 130

    print(json.dumps(asdict(result), indent=2))
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="sgz_storage_bench",
        description="TraceBoost SGZ / seismic-zfp benchmark runner",
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    check_env = subparsers.add_parser(
        "check-env",
        help="inspect whether the local Python environment can run SGZ benchmarks",
    )
    check_env.add_argument(
        "--seismic-zfp-repo",
        type=Path,
        default=DEFAULT_SEISMIC_ZFP_REPO,
        help="path to the local seismic-zfp clone",
    )

    bench = subparsers.add_parser(
        "benchmark-synthetic",
        help="benchmark SGZ on a synthetic seismic-like volume",
    )
    bench.add_argument("dataset_name", help="dataset label used in output paths")
    bench.add_argument("ilines", type=int)
    bench.add_argument("xlines", type=int)
    bench.add_argument("samples", type=int)
    bench.add_argument(
        "--bits-per-voxel",
        type=float,
        default=4.0,
        help="SGZ fixed-rate compression setting",
    )
    bench.add_argument(
        "--blockshape",
        default="4,4,-1",
        help="SGZ blockshape as il,xl,samples, allowing one -1 placeholder",
    )
    bench.add_argument(
        "--sample-interval-ms",
        type=float,
        default=2.0,
        help="synthetic sample interval in milliseconds",
    )
    bench.add_argument(
        "--scalar-factor",
        type=float,
        default=2.0,
        help="scalar factor for amplitude_scalar",
    )
    bench.add_argument(
        "--phase-rotation-degrees",
        type=float,
        default=35.0,
        help="phase rotation angle for the phase benchmark",
    )
    bench.add_argument(
        "--output-dir",
        type=Path,
        default=DEFAULT_TRACEBOOST_REPO / "tmp" / "sgz-bench",
        help="working directory for generated SGZ and output artifacts",
    )
    bench.add_argument(
        "--seismic-zfp-repo",
        type=Path,
        default=DEFAULT_SEISMIC_ZFP_REPO,
        help="path to the local seismic-zfp clone or installed package source tree",
    )
    bench.add_argument(
        "--traceboost-repo",
        type=Path,
        default=DEFAULT_TRACEBOOST_REPO,
        help="TraceBoost repo root used for optional tbvol transcode timing",
    )
    bench.add_argument(
        "--runtime-bin",
        type=Path,
        default=None,
        help="optional prebuilt seis-runtime binary path",
    )
    bench.add_argument(
        "--benchmark-transcode-to-tbvol",
        action="store_true",
        help="also measure SGZ -> SEG-Y -> tbvol timing if the runtime toolchain is available",
    )
    return parser


class BenchmarkError(RuntimeError):
    pass


def check_environment(seismic_zfp_repo: Path) -> EnvCheckResult:
    modules: dict[str, str] = {}
    for module_name in ("numpy", "segyio", "zfpy"):
        try:
            module = importlib.import_module(module_name)
            modules[module_name] = module_location(module)
        except Exception as error:  # pragma: no cover - environment-specific
            modules[module_name] = f"FAIL: {error!r}"

    try:
        load_seismic_zfp_bindings(seismic_zfp_repo)
        modules["seismic_zfp"] = str(seismic_zfp_repo)
    except Exception as error:  # pragma: no cover - environment-specific
        modules["seismic_zfp"] = f"FAIL: {error!r}"

    return EnvCheckResult(
        python_version=sys.version,
        python_executable=sys.executable,
        seismic_zfp_repo=str(seismic_zfp_repo),
        seismic_zfp_repo_exists=seismic_zfp_repo.exists(),
        cargo_found=shutil.which("cargo") is not None,
        modules=modules,
    )


def benchmark_synthetic(args: argparse.Namespace) -> BenchmarkResult:
    bindings = load_seismic_zfp_bindings(args.seismic_zfp_repo)
    blockshape_arg = parse_blockshape(args.blockshape)
    workdir = prepare_workdir(args.output_dir / args.dataset_name)
    sgz_path = workdir / f"{args.dataset_name}.sgz"

    creation_started = time.perf_counter()
    converter = bindings["StreamConverter"](
        str(sgz_path),
        ilines=np.arange(args.ilines, dtype=np.int32),
        xlines=np.arange(args.xlines, dtype=np.int32),
        samples=np.arange(args.samples, dtype=np.float32) * args.sample_interval_ms,
        bits_per_voxel=args.bits_per_voxel,
        blockshape=blockshape_arg,
    )
    resolved_blockshape = tuple(int(value) for value in converter.blockshape)
    stripe_ilines = resolved_blockshape[0]
    try:
        for iline_start in range(0, args.ilines, stripe_ilines):
            iline_stop = min(iline_start + stripe_ilines, args.ilines)
            converter.write(
                synthetic_chunk(
                    iline_start,
                    iline_stop,
                    args.ilines,
                    args.xlines,
                    args.samples,
                )
            )
    finally:
        converter.close()
    sgz_creation_ms = elapsed_ms(creation_started)

    with bindings["SgzReader"](str(sgz_path)) as reader:
        mid_inline = args.ilines // 2
        mid_xline = args.xlines // 2
        sample_interval_ms = float(args.sample_interval_ms)
        bandpass = benchmark_bandpass_corners(sample_interval_ms)

        inline_started = time.perf_counter()
        inline_section = np.asarray(reader.read_inline(mid_inline), dtype=np.float32)
        inline_section_read_ms = elapsed_ms(inline_started)

        xline_started = time.perf_counter()
        _xline_section = np.asarray(reader.read_crossline(mid_xline), dtype=np.float32)
        xline_section_read_ms = elapsed_ms(xline_started)

        preview_amplitude_scalar_ms = benchmark_preview(
            reader,
            mid_inline,
            lambda values: apply_amplitude_scalar(values, args.scalar_factor),
        )
        preview_trace_rms_normalize_ms = benchmark_preview(
            reader,
            mid_inline,
            apply_trace_rms_normalize,
        )
        preview_phase_rotation_ms = benchmark_preview(
            reader,
            mid_inline,
            lambda values: apply_phase_rotation(values, args.phase_rotation_degrees),
        )
        preview_bandpass_ms = benchmark_preview(
            reader,
            mid_inline,
            lambda values: apply_bandpass(values, sample_interval_ms, bandpass),
        )
        preview_bandpass_phase_rotation_ms = benchmark_preview(
            reader,
            mid_inline,
            lambda values: apply_phase_rotation(
                apply_bandpass(values, sample_interval_ms, bandpass),
                args.phase_rotation_degrees,
            ),
        )
        preview_pipeline_ms = benchmark_preview(
            reader,
            mid_inline,
            lambda values: apply_trace_rms_normalize(
                apply_amplitude_scalar(values, args.scalar_factor)
            ),
        )

        decode_mae, decode_max_abs = measure_decode_drift(
            reader,
            args.ilines,
            args.xlines,
            args.samples,
            stripe_ilines,
        )

        apply_amplitude_scalar_ms = benchmark_apply(
            reader,
            workdir / "apply-amplitude.bin",
            args.ilines,
            args.xlines,
            args.samples,
            stripe_ilines,
            lambda values: apply_amplitude_scalar(values, args.scalar_factor),
        )
        apply_trace_rms_normalize_ms = benchmark_apply(
            reader,
            workdir / "apply-rms.bin",
            args.ilines,
            args.xlines,
            args.samples,
            stripe_ilines,
            apply_trace_rms_normalize,
        )
        apply_phase_rotation_ms = benchmark_apply(
            reader,
            workdir / "apply-phase.bin",
            args.ilines,
            args.xlines,
            args.samples,
            stripe_ilines,
            lambda values: apply_phase_rotation(values, args.phase_rotation_degrees),
        )
        apply_bandpass_ms = benchmark_apply(
            reader,
            workdir / "apply-bandpass.bin",
            args.ilines,
            args.xlines,
            args.samples,
            stripe_ilines,
            lambda values: apply_bandpass(values, sample_interval_ms, bandpass),
        )
        apply_bandpass_phase_rotation_ms = benchmark_apply(
            reader,
            workdir / "apply-bandpass-phase.bin",
            args.ilines,
            args.xlines,
            args.samples,
            stripe_ilines,
            lambda values: apply_phase_rotation(
                apply_bandpass(values, sample_interval_ms, bandpass),
                args.phase_rotation_degrees,
            ),
        )
        pipeline_output_path = workdir / "apply-pipeline.bin"
        apply_pipeline_ms = benchmark_apply(
            reader,
            pipeline_output_path,
            args.ilines,
            args.xlines,
            args.samples,
            stripe_ilines,
            lambda values: apply_trace_rms_normalize(
                apply_amplitude_scalar(values, args.scalar_factor)
            ),
        )

    sgz_to_segy_ms = None
    segy_to_tbvol_ms = None
    sgz_to_tbvol_total_ms = None
    tbvol_bytes = None
    tbvol_file_count = None

    if args.benchmark_transcode_to_tbvol:
        (
            sgz_to_segy_ms,
            segy_to_tbvol_ms,
            sgz_to_tbvol_total_ms,
            tbvol_bytes,
            tbvol_file_count,
        ) = benchmark_transcode_to_tbvol(
            bindings,
            sgz_path,
            workdir,
            args.traceboost_repo,
            args.runtime_bin,
        )

    input_store_bytes, input_file_count = path_metrics(sgz_path)
    pipeline_output_bytes, pipeline_output_file_count = path_metrics(pipeline_output_path)

    return BenchmarkResult(
        dataset_name=args.dataset_name,
        shape=[args.ilines, args.xlines, args.samples],
        bits_per_voxel=float(args.bits_per_voxel),
        blockshape=list(resolved_blockshape),
        sample_interval_ms=float(args.sample_interval_ms),
        scalar_factor=float(args.scalar_factor),
        phase_rotation_degrees=float(args.phase_rotation_degrees),
        bandpass_hz=[float(value) for value in bandpass],
        sgz_creation_ms=sgz_creation_ms,
        input_store_bytes=input_store_bytes,
        input_file_count=input_file_count,
        inline_section_read_ms=inline_section_read_ms,
        xline_section_read_ms=xline_section_read_ms,
        preview_amplitude_scalar_ms=preview_amplitude_scalar_ms,
        preview_trace_rms_normalize_ms=preview_trace_rms_normalize_ms,
        preview_phase_rotation_ms=preview_phase_rotation_ms,
        preview_bandpass_ms=preview_bandpass_ms,
        preview_bandpass_phase_rotation_ms=preview_bandpass_phase_rotation_ms,
        preview_pipeline_ms=preview_pipeline_ms,
        apply_amplitude_scalar_ms=apply_amplitude_scalar_ms,
        apply_trace_rms_normalize_ms=apply_trace_rms_normalize_ms,
        apply_phase_rotation_ms=apply_phase_rotation_ms,
        apply_bandpass_ms=apply_bandpass_ms,
        apply_bandpass_phase_rotation_ms=apply_bandpass_phase_rotation_ms,
        apply_pipeline_ms=apply_pipeline_ms,
        pipeline_output_bytes=pipeline_output_bytes,
        pipeline_output_file_count=pipeline_output_file_count,
        decode_volume_mean_abs_error=decode_mae,
        decode_volume_max_abs_error=decode_max_abs,
        sgz_to_segy_ms=sgz_to_segy_ms,
        segy_to_tbvol_ms=segy_to_tbvol_ms,
        sgz_to_tbvol_total_ms=sgz_to_tbvol_total_ms,
        tbvol_bytes=tbvol_bytes,
        tbvol_file_count=tbvol_file_count,
    )


def load_seismic_zfp_bindings(seismic_zfp_repo: Path) -> dict[str, Any]:
    import importlib.metadata as importlib_metadata

    repo = seismic_zfp_repo.resolve()
    if not repo.exists():
        raise BenchmarkError(f"seismic-zfp repo not found: {repo}")

    module_names = [
        "seismic_zfp",
        "seismic_zfp.conversion",
        "seismic_zfp.read",
    ]
    for module_name in module_names:
        sys.modules.pop(module_name, None)

    original_version = importlib_metadata.version

    def patched_version(distribution_name: str) -> str:
        normalized = distribution_name.replace("-", "_").lower()
        if normalized == "seismic_zfp":
            return "0.2.4"
        return original_version(distribution_name)

    importlib_metadata.version = patched_version
    if str(repo) not in sys.path:
        sys.path.insert(0, str(repo))
    try:
        conversion_module = importlib.import_module("seismic_zfp.conversion")
        read_module = importlib.import_module("seismic_zfp.read")
    except ModuleNotFoundError as error:
        missing = error.name or "unknown"
        raise BenchmarkError(
            "missing Python dependency for SGZ benchmark: "
            f"{missing}. Install seismic-zfp prerequisites such as segyio and zfpy first."
        ) from error
    except Exception as error:
        raise BenchmarkError(f"failed to import seismic-zfp from {repo}: {error!r}") from error

    return {
        "StreamConverter": conversion_module.StreamConverter,
        "SgzConverter": conversion_module.SgzConverter,
        "SgzReader": read_module.SgzReader,
    }


def parse_blockshape(value: str) -> tuple[int, int, int]:
    try:
        parts = [int(part.strip()) for part in value.split(",")]
    except ValueError as error:
        raise BenchmarkError(f"invalid blockshape '{value}': {error}") from error
    if len(parts) != 3:
        raise BenchmarkError(
            f"invalid blockshape '{value}': expected exactly three comma-separated integers"
        )
    return parts[0], parts[1], parts[2]


def benchmark_preview(
    reader: Any,
    mid_inline: int,
    processor: Callable[[np.ndarray], np.ndarray],
) -> float:
    started = time.perf_counter()
    section = np.asarray(reader.read_inline(mid_inline), dtype=np.float32)
    _ = processor(section)
    return elapsed_ms(started)


def benchmark_apply(
    reader: Any,
    output_path: Path,
    ilines: int,
    xlines: int,
    samples: int,
    stripe_ilines: int,
    processor: Callable[[np.ndarray], np.ndarray],
) -> float:
    output_path.parent.mkdir(parents=True, exist_ok=True)
    started = time.perf_counter()
    with output_path.open("wb") as handle:
        for iline_start in range(0, ilines, stripe_ilines):
            iline_stop = min(iline_start + stripe_ilines, ilines)
            chunk = np.asarray(
                reader.read_subvolume(iline_start, iline_stop, 0, xlines, 0, samples),
                dtype=np.float32,
            )
            traces = chunk.reshape((-1, samples))
            processed = processor(traces).reshape(chunk.shape)
            processed.astype("<f4", copy=False).tofile(handle)
    return elapsed_ms(started)


def measure_decode_drift(
    reader: Any,
    ilines: int,
    xlines: int,
    samples: int,
    stripe_ilines: int,
) -> tuple[float, float]:
    total_abs_error = 0.0
    total_values = 0
    max_abs_error = 0.0

    for iline_start in range(0, ilines, stripe_ilines):
        iline_stop = min(iline_start + stripe_ilines, ilines)
        decoded = np.asarray(
            reader.read_subvolume(iline_start, iline_stop, 0, xlines, 0, samples),
            dtype=np.float32,
        )
        expected = synthetic_chunk(iline_start, iline_stop, ilines, xlines, samples)
        error = np.abs(decoded - expected).astype(np.float32, copy=False)
        total_abs_error += float(error.sum(dtype=np.float64))
        total_values += int(error.size)
        max_abs_error = max(max_abs_error, float(error.max(initial=0.0)))

    mean_abs_error = total_abs_error / max(total_values, 1)
    return mean_abs_error, max_abs_error


def benchmark_transcode_to_tbvol(
    bindings: dict[str, Any],
    sgz_path: Path,
    workdir: Path,
    traceboost_repo: Path,
    runtime_bin: Path | None,
) -> tuple[float, float, float, int, int]:
    resolved_runtime_bin = resolve_runtime_bin(traceboost_repo, runtime_bin)
    segy_path = workdir / f"{sgz_path.stem}.roundtrip.sgy"
    tbvol_path = workdir / f"{sgz_path.stem}.tbvol"

    sgz_to_segy_ms = write_segy_from_sgz(bindings["SgzReader"], sgz_path, segy_path)

    ingest_command = [
        str(resolved_runtime_bin),
        "ingest",
        str(segy_path),
        str(tbvol_path),
    ]
    segy_to_tbvol_started = time.perf_counter()
    completed = subprocess.run(
        ingest_command,
        cwd=traceboost_repo,
        capture_output=True,
        text=True,
        check=False,
    )
    segy_to_tbvol_ms = elapsed_ms(segy_to_tbvol_started)
    if completed.returncode != 0:
        raise BenchmarkError(
            "failed to ingest SEG-Y into tbvol via seis-runtime: "
            f"{completed.stderr.strip() or completed.stdout.strip()}"
        )

    tbvol_bytes, tbvol_file_count = path_metrics(tbvol_path)
    return (
        sgz_to_segy_ms,
        segy_to_tbvol_ms,
        sgz_to_segy_ms + segy_to_tbvol_ms,
        tbvol_bytes,
        tbvol_file_count,
    )


def write_segy_from_sgz(
    sgz_reader_type: Any,
    sgz_path: Path,
    segy_path: Path,
) -> float:
    import segyio

    started = time.perf_counter()
    with sgz_reader_type(str(sgz_path)) as reader:
        sample_interval_us = resolve_sample_interval_us_from_reader(reader)
        delay_recording_time_ms = int(round(float(reader.zslices[0]))) if len(reader.zslices) else 0
        spec = segyio.spec()
        spec.samples = list(reader.zslices)
        spec.ilines = [int(value) for value in reader.ilines]
        spec.xlines = [int(value) for value in reader.xlines]
        spec.offsets = [0]
        spec.sorting = 2
        spec.format = 5

        with segyio.create(str(segy_path), spec) as segyfile:
            trace_index = 0
            for iline_index, iline_number in enumerate(reader.ilines):
                inline = np.asarray(reader.read_inline(iline_index), dtype=np.float32)
                for xline_index, xline_number in enumerate(reader.xlines):
                    segyfile.trace[trace_index] = inline[xline_index, :]
                    segyfile.header[trace_index] = {
                        segyio.TraceField.INLINE_3D: int(iline_number),
                        segyio.TraceField.CROSSLINE_3D: int(xline_number),
                    }
                    trace_index += 1

        patch_synthetic_segy_headers(
            segy_path,
            [int(value) for value in reader.ilines],
            [int(value) for value in reader.xlines],
            int(reader.n_samples),
            sample_interval_us,
            delay_recording_time_ms,
        )

    return elapsed_ms(started)


def resolve_sample_interval_us_from_reader(reader: Any) -> int:
    if len(reader.zslices) >= 2:
        delta_ms = float(reader.zslices[1]) - float(reader.zslices[0])
        resolved_us = int(round(delta_ms * 1000.0))
        if resolved_us > 0:
            return resolved_us
    return 2000


def patch_synthetic_segy_headers(
    segy_path: Path,
    ilines: list[int],
    xlines: list[int],
    sample_count: int,
    sample_interval_us: int,
    delay_recording_time_ms: int,
) -> None:
    trace_stride = 240 + (sample_count * 4)
    with segy_path.open("r+b") as handle:
        patch_u16_be(handle, 3216, sample_interval_us)
        patch_u16_be(handle, 3220, sample_count)
        patch_u16_be(handle, 3224, 5)

        trace_index = 0
        for iline_number in ilines:
            for xline_number in xlines:
                trace_header_offset = 3600 + (trace_index * trace_stride)
                patch_i32_be(handle, trace_header_offset + 188, iline_number)
                patch_i32_be(handle, trace_header_offset + 192, xline_number)
                patch_i16_be(handle, trace_header_offset + 108, delay_recording_time_ms)
                patch_u16_be(handle, trace_header_offset + 114, sample_count)
                patch_u16_be(handle, trace_header_offset + 116, sample_interval_us)
                trace_index += 1


def patch_u16_be(handle: Any, offset: int, value: int) -> None:
    handle.seek(offset)
    handle.write(int(value).to_bytes(2, byteorder="big", signed=False))


def patch_i16_be(handle: Any, offset: int, value: int) -> None:
    handle.seek(offset)
    handle.write(int(value).to_bytes(2, byteorder="big", signed=True))


def patch_i32_be(handle: Any, offset: int, value: int) -> None:
    handle.seek(offset)
    handle.write(int(value).to_bytes(4, byteorder="big", signed=True))


def resolve_runtime_bin(traceboost_repo: Path, runtime_bin: Path | None) -> Path:
    if runtime_bin is not None:
        resolved = runtime_bin.resolve()
        if not resolved.exists():
            raise BenchmarkError(f"runtime binary not found: {resolved}")
        return resolved

    default_bin = traceboost_repo / RUNTIME_BIN_RELATIVE
    if default_bin.exists():
        return default_bin

    cargo = shutil.which("cargo")
    if cargo is None:
        raise BenchmarkError(
            "cargo is not available and no prebuilt --runtime-bin was provided"
        )

    build = subprocess.run(
        [cargo, "build", "--release", "-p", "seis-runtime"],
        cwd=traceboost_repo,
        capture_output=True,
        text=True,
        check=False,
    )
    if build.returncode != 0:
        raise BenchmarkError(
            "failed to build seis-runtime: "
            f"{build.stderr.strip() or build.stdout.strip()}"
        )
    if not default_bin.exists():
        raise BenchmarkError(f"expected runtime binary was not built: {default_bin}")
    return default_bin


def synthetic_chunk(
    iline_start: int,
    iline_stop: int,
    ilines: int,
    xlines: int,
    samples: int,
) -> np.ndarray:
    il = (
        np.arange(iline_start, iline_stop, dtype=np.float32)[:, None, None]
        / float(max(ilines, 1))
    )
    xl = np.arange(xlines, dtype=np.float32)[None, :, None] / float(max(xlines, 1))
    smp = np.arange(samples, dtype=np.float32)[None, None, :] / float(max(samples, 1))
    values = ((np.sin(il * 17.0) + np.cos(xl * 11.0)) * (1.0 - smp)) + (
        np.sin(smp * 31.0) * 0.35
    )
    return values.astype(np.float32, copy=False)


def apply_amplitude_scalar(values: np.ndarray, scalar_factor: float) -> np.ndarray:
    return np.asarray(values * scalar_factor, dtype=np.float32)


def apply_trace_rms_normalize(values: np.ndarray) -> np.ndarray:
    rms = np.sqrt(np.mean(np.square(values, dtype=np.float32), axis=1, keepdims=True))
    divisor = np.maximum(rms, np.float32(1.0e-8)).astype(np.float32, copy=False)
    return np.asarray(values / divisor, dtype=np.float32)


def apply_phase_rotation(values: np.ndarray, angle_degrees: float) -> np.ndarray:
    radians = math.radians(angle_degrees)
    spectrum = np.fft.rfft(values, axis=1)
    spectrum *= np.exp(1j * radians)
    spectrum[:, 0] = spectrum[:, 0].real + 0j
    if values.shape[1] % 2 == 0:
        spectrum[:, -1] = spectrum[:, -1].real + 0j
    rotated = np.fft.irfft(spectrum, n=values.shape[1], axis=1)
    return np.asarray(rotated, dtype=np.float32)


def apply_bandpass(
    values: np.ndarray,
    sample_interval_ms: float,
    corners_hz: Iterable[float],
) -> np.ndarray:
    f1_hz, f2_hz, f3_hz, f4_hz = [float(value) for value in corners_hz]
    samples = values.shape[1]
    dt_s = sample_interval_ms / 1000.0
    freqs = np.fft.rfftfreq(samples, d=dt_s)
    response = np.zeros_like(freqs, dtype=np.float32)

    if f2_hz > f1_hz:
        low_taper = (freqs >= f1_hz) & (freqs < f2_hz)
        low_phase = (freqs[low_taper] - f1_hz) / max(f2_hz - f1_hz, 1.0e-8)
        response[low_taper] = 0.5 - (0.5 * np.cos(np.pi * low_phase))
    response[(freqs >= f2_hz) & (freqs <= f3_hz)] = 1.0
    if f4_hz > f3_hz:
        high_taper = (freqs > f3_hz) & (freqs <= f4_hz)
        high_phase = (freqs[high_taper] - f3_hz) / max(f4_hz - f3_hz, 1.0e-8)
        response[high_taper] = 0.5 + (0.5 * np.cos(np.pi * high_phase))

    spectrum = np.fft.rfft(values, axis=1)
    filtered = np.fft.irfft(spectrum * response[None, :], n=samples, axis=1)
    return np.asarray(filtered, dtype=np.float32)


def benchmark_bandpass_corners(sample_interval_ms: float) -> tuple[float, float, float, float]:
    nyquist_hz = 500.0 / max(sample_interval_ms, np.finfo(np.float32).eps)
    f1_hz = max(nyquist_hz * 0.06, 4.0)
    f2_hz = max(nyquist_hz * 0.10, f1_hz + 1.0)
    f4_hz = min(max(nyquist_hz * 0.45, f2_hz + 6.0), nyquist_hz)
    f3_hz = min(max(nyquist_hz * 0.32, f2_hz + 4.0), f4_hz)
    return float(f1_hz), float(f2_hz), float(f3_hz), float(f4_hz)


def prepare_workdir(path: Path) -> Path:
    if path.exists():
        shutil.rmtree(path)
    path.mkdir(parents=True, exist_ok=True)
    return path


def path_metrics(path: Path) -> tuple[int, int]:
    if not path.exists():
        return 0, 0
    if path.is_file():
        return path.stat().st_size, 1

    total_bytes = 0
    total_files = 0
    for child in path.rglob("*"):
        if child.is_file():
            total_bytes += child.stat().st_size
            total_files += 1
    return total_bytes, total_files


def elapsed_ms(started: float) -> float:
    return (time.perf_counter() - started) * 1000.0


def module_location(module: Any) -> str:
    return getattr(module, "__file__", "<builtin>")


if __name__ == "__main__":
    raise SystemExit(main())

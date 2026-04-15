#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import math
import pathlib
import subprocess
from dataclasses import dataclass


@dataclass(frozen=True)
class SurveySpec:
    inline_count: int
    xline_count: int
    sample_count: int
    sample_interval_ms: float
    sample_start_ms: float
    origin_x: float
    origin_y: float
    inline_dx: float
    inline_dy: float
    xline_dx: float
    xline_dy: float
    coordinate_mode: str
    source: str

    @property
    def max_time_ms(self) -> float:
        return self.sample_start_ms + self.sample_interval_ms * max(self.sample_count - 1, 0)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Generate synthetic Vint and paired TWT/depth horizons for an F3-scale survey."
    )
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--store-path", type=pathlib.Path)
    source.add_argument("--segy-path", type=pathlib.Path)
    parser.add_argument("--repo-root", type=pathlib.Path, default=pathlib.Path(__file__).resolve().parents[1])
    parser.add_argument("--output-dir", type=pathlib.Path, required=True)
    parser.add_argument("--horizon-count", type=int, default=4)
    parser.add_argument("--profile-stride-inline", type=int, default=80)
    parser.add_argument("--profile-stride-xline", type=int, default=100)
    return parser.parse_args()


def load_spec(args: argparse.Namespace) -> SurveySpec:
    if args.store_path:
        manifest = json.loads((args.store_path / "manifest.json").read_text())
        volume = manifest["volume"]
        axes = volume["axes"]
        spatial = volume.get("spatial") or {}
        transform = spatial.get("grid_transform")
        if not transform:
            raise SystemExit("store manifest does not contain spatial.grid_transform")
        sample_axis = axes["sample_axis_ms"]
        if not sample_axis:
            raise SystemExit("store manifest sample axis is empty")
        sample_interval_ms = sample_axis[1] - sample_axis[0] if len(sample_axis) >= 2 else 0.0
        return SurveySpec(
            inline_count=len(axes["ilines"]),
            xline_count=len(axes["xlines"]),
            sample_count=len(sample_axis),
            sample_interval_ms=sample_interval_ms,
            sample_start_ms=float(sample_axis[0]),
            origin_x=float(transform["origin"]["x"]),
            origin_y=float(transform["origin"]["y"]),
            inline_dx=float(transform["inline_basis"]["x"]),
            inline_dy=float(transform["inline_basis"]["y"]),
            xline_dx=float(transform["xline_basis"]["x"]),
            xline_dy=float(transform["xline_basis"]["y"]),
            coordinate_mode="survey_crs",
            source=str(args.store_path),
        )

    inspect = json.loads(
        subprocess.check_output(
            ["cargo", "run", "-p", "traceboost-app", "--", "inspect", str(args.segy_path)],
            cwd=args.repo_root,
            text=True,
        )
    )
    analyze = json.loads(
        subprocess.check_output(
            ["cargo", "run", "-p", "traceboost-app", "--", "analyze", str(args.segy_path)],
            cwd=args.repo_root,
            text=True,
        )
    )
    sample_interval_ms = inspect["sample_interval_us"] / 1000.0
    return SurveySpec(
        inline_count=int(analyze["geometry"]["inline_count"]),
        xline_count=int(analyze["geometry"]["crossline_count"]),
        sample_count=int(inspect["samples_per_trace"]),
        sample_interval_ms=sample_interval_ms,
        sample_start_ms=0.0,
        origin_x=0.0,
        origin_y=0.0,
        inline_dx=1.0,
        inline_dy=0.0,
        xline_dx=0.0,
        xline_dy=1.0,
        coordinate_mode="logical_grid",
        source=str(args.segy_path),
    )


def clamp(value: float, low: float, high: float) -> float:
    return max(low, min(high, value))


def grid_xy(spec: SurveySpec, inline_index: int, xline_index: int) -> tuple[float, float]:
    x = spec.origin_x + inline_index * spec.inline_dx + xline_index * spec.xline_dx
    y = spec.origin_y + inline_index * spec.inline_dy + xline_index * spec.xline_dy
    return x, y


def horizon_times_ms(spec: SurveySpec, horizon_count: int, u: float, v: float) -> list[float]:
    max_time = spec.max_time_ms
    fault = 18.0 if v > 0.58 else 0.0
    fault += 8.0 * math.sin(u * math.pi) if v > 0.58 else 0.0
    undulation = 28.0 * math.sin((u * 1.7 + v * 0.6) * math.pi)
    drift = 40.0 * (u - 0.5) + 24.0 * (v - 0.5)

    times: list[float] = []
    current = 220.0 + undulation * 0.25 + drift * 0.15 + fault
    for idx in range(horizon_count):
        thickness = 180.0 + idx * 55.0
        thickness += 24.0 * math.sin((idx + 1) * u * math.pi * 1.3)
        thickness += 18.0 * math.cos((idx + 1) * v * math.pi * 1.1)
        thickness += 6.0 * fault
        current += thickness
        remaining = horizon_count - idx - 1
        max_allowed = max_time - remaining * 140.0
        current = clamp(current, spec.sample_start_ms + 60.0 * (idx + 1), max_allowed)
        times.append(current)
    return times


def interval_velocities_mps(horizon_count: int, u: float, v: float) -> list[float]:
    values = []
    for idx in range(horizon_count + 1):
        base = 1800.0 + idx * 340.0
        lateral = 110.0 * math.sin((idx + 1) * u * math.pi)
        lateral += 90.0 * math.cos((idx + 2) * v * math.pi * 0.7)
        fault = 70.0 if v > 0.58 and idx >= 1 else 0.0
        values.append(base + lateral + fault)
    return values


def cumulative_depths_m(times_ms: list[float], vint_mps: list[float]) -> list[float]:
    depths: list[float] = []
    previous_time = 0.0
    depth = 0.0
    for idx, time_ms in enumerate(times_ms):
        delta_time_s = (time_ms - previous_time) / 1000.0
        depth += 0.5 * vint_mps[idx] * delta_time_s
        depths.append(depth)
        previous_time = time_ms
    return depths


def profile_samples(times_ms: list[float], vint_mps: list[float]) -> list[tuple[float, float, float, float]]:
    samples = [(0.0, 0.0, vint_mps[0], vint_mps[0])]
    cumulative_time_s = 0.0
    cumulative_v2_t = 0.0
    depth = 0.0
    previous_time = 0.0
    for idx, time_ms in enumerate(times_ms):
        delta_time_s = (time_ms - previous_time) / 1000.0
        cumulative_time_s += delta_time_s
        cumulative_v2_t += (vint_mps[idx] ** 2) * delta_time_s
        depth += 0.5 * vint_mps[idx] * delta_time_s
        vrms = math.sqrt(cumulative_v2_t / cumulative_time_s)
        vavg = (2.0 * depth / cumulative_time_s) if cumulative_time_s > 0 else vint_mps[idx]
        samples.append((time_ms, vrms, vint_mps[idx], vavg))
        previous_time = time_ms
    return [(time_ms, vrms, vint, vavg, depth_at_time_ms(time_ms, times_ms, vint_mps)) for time_ms, vrms, vint, vavg in samples]


def depth_at_time_ms(target_time_ms: float, times_ms: list[float], vint_mps: list[float]) -> float:
    previous_time = 0.0
    depth = 0.0
    for idx, boundary_ms in enumerate(times_ms):
        capped = min(target_time_ms, boundary_ms)
        if capped <= previous_time:
            break
        depth += 0.5 * vint_mps[idx] * ((capped - previous_time) / 1000.0)
        if target_time_ms <= boundary_ms:
            return depth
        previous_time = boundary_ms
    if target_time_ms > previous_time:
        depth += 0.5 * vint_mps[len(times_ms)] * ((target_time_ms - previous_time) / 1000.0)
    return depth


def emit_horizons(spec: SurveySpec, output_dir: pathlib.Path, horizon_count: int) -> None:
    twt_files = [output_dir / f"horizon_{idx + 1:02d}_twt_ms.xyz" for idx in range(horizon_count)]
    depth_m_files = [output_dir / f"horizon_{idx + 1:02d}_depth_m.xyz" for idx in range(horizon_count)]
    depth_ft_files = [output_dir / f"horizon_{idx + 1:02d}_depth_ft.xyz" for idx in range(horizon_count)]
    handles = [path.open("w", encoding="utf-8") for path in [*twt_files, *depth_m_files, *depth_ft_files]]
    try:
        for inline_index in range(spec.inline_count):
            u = 0.0 if spec.inline_count <= 1 else inline_index / (spec.inline_count - 1)
            for xline_index in range(spec.xline_count):
                v = 0.0 if spec.xline_count <= 1 else xline_index / (spec.xline_count - 1)
                x, y = grid_xy(spec, inline_index, xline_index)
                times_ms = horizon_times_ms(spec, horizon_count, u, v)
                vint_mps = interval_velocities_mps(horizon_count, u, v)
                depths_m = cumulative_depths_m(times_ms, vint_mps)
                for idx in range(horizon_count):
                    handles[idx].write(f"{x:.3f} {y:.3f} {times_ms[idx]:.3f}\n")
                    handles[horizon_count + idx].write(f"{x:.3f} {y:.3f} {depths_m[idx]:.3f}\n")
                    handles[2 * horizon_count + idx].write(f"{x:.3f} {y:.3f} {depths_m[idx] * 3.280839895:.3f}\n")
    finally:
        for handle in handles:
            handle.close()


def emit_velocity_functions(spec: SurveySpec, output_dir: pathlib.Path, horizon_count: int, stride_inline: int, stride_xline: int) -> None:
    profile_indices_inline = sorted({0, spec.inline_count - 1, *range(0, spec.inline_count, max(stride_inline, 1))})
    profile_indices_xline = sorted({0, spec.xline_count - 1, *range(0, spec.xline_count, max(stride_xline, 1))})
    lines = ["CDP-X       CDP-Y   Time(ms)  Vrms    Vint    Vavg   Depth(m)"]
    for inline_index in profile_indices_inline:
        u = 0.0 if spec.inline_count <= 1 else inline_index / (spec.inline_count - 1)
        for xline_index in profile_indices_xline:
            v = 0.0 if spec.xline_count <= 1 else xline_index / (spec.xline_count - 1)
            x, y = grid_xy(spec, inline_index, xline_index)
            times_ms = horizon_times_ms(spec, horizon_count, u, v)
            vint_mps = interval_velocities_mps(horizon_count, u, v)
            for time_ms, vrms, vint, vavg, depth_m in profile_samples(times_ms, vint_mps):
                lines.append(
                    f"{x:10.3f} {y:10.3f} {time_ms:9.2f} {vrms:7.2f} {vint:7.2f} {vavg:7.2f} {depth_m:9.2f}"
                )
    (output_dir / "Velocity_functions.txt").write_text("\n".join(lines) + "\n", encoding="utf-8")


def emit_spec(spec: SurveySpec, output_dir: pathlib.Path, horizon_count: int) -> None:
    payload = {
        "source": spec.source,
        "coordinate_mode": spec.coordinate_mode,
        "inline_count": spec.inline_count,
        "xline_count": spec.xline_count,
        "sample_count": spec.sample_count,
        "sample_interval_ms": spec.sample_interval_ms,
        "sample_start_ms": spec.sample_start_ms,
        "max_time_ms": spec.max_time_ms,
        "horizon_count": horizon_count,
        "grid_transform": {
            "origin": {"x": spec.origin_x, "y": spec.origin_y},
            "inline_basis": {"x": spec.inline_dx, "y": spec.inline_dy},
            "xline_basis": {"x": spec.xline_dx, "y": spec.xline_dy},
        },
        "notes": [
            "Time horizons are written in canonical ms.",
            "Depth horizons are written in canonical m and export convenience ft.",
            "Velocity_functions.txt is sparse by design; horizons are dense truth on the full survey grid.",
            "If coordinate_mode is logical_grid, import the horizons only after mapping them into the active survey CRS or generate from a store manifest instead.",
        ],
    }
    (output_dir / "survey_spec.json").write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


def main() -> None:
    args = parse_args()
    spec = load_spec(args)
    output_dir = args.output_dir
    output_dir.mkdir(parents=True, exist_ok=True)
    emit_spec(spec, output_dir, args.horizon_count)
    emit_velocity_functions(
        spec,
        output_dir,
        args.horizon_count,
        args.profile_stride_inline,
        args.profile_stride_xline,
    )
    emit_horizons(spec, output_dir, args.horizon_count)
    print(json.dumps({
        "output_dir": str(output_dir),
        "inline_count": spec.inline_count,
        "xline_count": spec.xline_count,
        "sample_count": spec.sample_count,
        "sample_interval_ms": spec.sample_interval_ms,
        "coordinate_mode": spec.coordinate_mode,
        "horizon_count": args.horizon_count,
    }, indent=2))


if __name__ == "__main__":
    main()

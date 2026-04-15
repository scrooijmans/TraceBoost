from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Sequence

from .client import DEFAULT_MANIFEST_PATH, REPO_ROOT, TraceBoostApp, TraceBoostCommandError


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Python automation wrapper around traceboost-app")
    parser.add_argument("--repo-root", default=str(REPO_ROOT))
    parser.add_argument("--manifest-path", default=str(DEFAULT_MANIFEST_PATH))
    parser.add_argument("--binary", default=None)

    subparsers = parser.add_subparsers(dest="command", required=True)

    subparsers.add_parser("backend-info")
    subparsers.add_parser("operation-catalog")

    preflight = subparsers.add_parser("preflight-import")
    preflight.add_argument("input")
    add_header_options(preflight)

    import_dataset_parser = subparsers.add_parser("import-dataset")
    import_dataset_parser.add_argument("input")
    import_dataset_parser.add_argument("output")
    add_header_options(import_dataset_parser)
    import_dataset_parser.add_argument("--overwrite-existing", action="store_true")

    open_dataset = subparsers.add_parser("open-dataset")
    open_dataset.add_argument("store")

    set_native_coordinate_reference = subparsers.add_parser("set-native-coordinate-reference")
    set_native_coordinate_reference.add_argument("store")
    set_native_coordinate_reference.add_argument("--coordinate-reference-id")
    set_native_coordinate_reference.add_argument("--coordinate-reference-name")

    resolve_survey_map = subparsers.add_parser("resolve-survey-map")
    resolve_survey_map.add_argument("store")
    resolve_survey_map.add_argument("--display-coordinate-reference-id")

    export_segy = subparsers.add_parser("export-segy")
    export_segy.add_argument("store")
    export_segy.add_argument("output")
    export_segy.add_argument("--overwrite-existing", action="store_true")

    export_zarr = subparsers.add_parser("export-zarr")
    export_zarr.add_argument("store")
    export_zarr.add_argument("output")
    export_zarr.add_argument("--overwrite-existing", action="store_true")

    import_horizons = subparsers.add_parser("import-horizons")
    import_horizons.add_argument("store")
    import_horizons.add_argument("inputs", nargs="+")
    import_horizons.add_argument("--source-coordinate-reference-id")
    import_horizons.add_argument("--source-coordinate-reference-name")
    import_horizons.add_argument("--assume-same-as-survey", action="store_true")

    view_section = subparsers.add_parser("view-section")
    view_section.add_argument("store")
    view_section.add_argument("axis", choices=["inline", "xline"])
    view_section.add_argument("index", type=int)

    view_section_horizons = subparsers.add_parser("view-section-horizons")
    view_section_horizons.add_argument("store")
    view_section_horizons.add_argument("axis", choices=["inline", "xline"])
    view_section_horizons.add_argument("index", type=int)

    load_velocity_models = subparsers.add_parser("load-velocity-models")
    load_velocity_models.add_argument("store")

    ensure_demo_transform = subparsers.add_parser("ensure-demo-survey-time-depth-transform")
    ensure_demo_transform.add_argument("store")

    import_velocity_functions = subparsers.add_parser("import-velocity-functions-model")
    import_velocity_functions.add_argument("store")
    import_velocity_functions.add_argument("input")
    import_velocity_functions.add_argument(
        "--velocity-kind",
        default="interval",
        choices=["interval", "average", "rms"]
    )

    prepare_survey_demo = subparsers.add_parser("prepare-survey-demo")
    prepare_survey_demo.add_argument("store")
    prepare_survey_demo.add_argument("--display-coordinate-reference-id")

    subparsers.add_parser("verify-surface-contracts")

    return parser


def add_header_options(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--inline-byte", type=int)
    parser.add_argument("--inline-type", default="i32", choices=["i16", "i32"])
    parser.add_argument("--crossline-byte", type=int)
    parser.add_argument("--crossline-type", default="i32", choices=["i16", "i32"])
    parser.add_argument("--third-axis-byte", type=int)
    parser.add_argument("--third-axis-type", default="i32", choices=["i16", "i32"])


def app_from_args(args: argparse.Namespace) -> TraceBoostApp:
    return TraceBoostApp(
        repo_root=Path(args.repo_root),
        manifest_path=Path(args.manifest_path),
        binary=args.binary
    )


def main(argv: Sequence[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(list(argv) if argv is not None else None)
    app = app_from_args(args)

    try:
        if args.command == "backend-info":
            result = app.backend_info()
        elif args.command == "operation-catalog":
            result = app.operation_catalog()
        elif args.command == "preflight-import":
            result = app.preflight_import(
                args.input,
                inline_byte=args.inline_byte,
                crossline_byte=args.crossline_byte,
                third_axis_byte=args.third_axis_byte,
                inline_type=args.inline_type,
                crossline_type=args.crossline_type,
                third_axis_type=args.third_axis_type
            )
        elif args.command == "import-dataset":
            result = app.import_dataset(
                args.input,
                args.output,
                overwrite_existing=args.overwrite_existing,
                inline_byte=args.inline_byte,
                crossline_byte=args.crossline_byte,
                third_axis_byte=args.third_axis_byte,
                inline_type=args.inline_type,
                crossline_type=args.crossline_type,
                third_axis_type=args.third_axis_type
            )
        elif args.command == "open-dataset":
            result = app.open_dataset(args.store)
        elif args.command == "set-native-coordinate-reference":
            result = app.set_native_coordinate_reference(
                args.store,
                coordinate_reference_id=args.coordinate_reference_id,
                coordinate_reference_name=args.coordinate_reference_name
            )
        elif args.command == "resolve-survey-map":
            result = app.resolve_survey_map(
                args.store,
                display_coordinate_reference_id=args.display_coordinate_reference_id
            )
        elif args.command == "export-segy":
            result = app.export_segy(args.store, args.output, overwrite_existing=args.overwrite_existing)
        elif args.command == "export-zarr":
            result = app.export_zarr(args.store, args.output, overwrite_existing=args.overwrite_existing)
        elif args.command == "import-horizons":
            result = app.import_horizons(
                args.store,
                args.inputs,
                source_coordinate_reference_id=args.source_coordinate_reference_id,
                source_coordinate_reference_name=args.source_coordinate_reference_name,
                assume_same_as_survey=args.assume_same_as_survey
            )
        elif args.command == "view-section":
            result = app.view_section(args.store, args.axis, args.index)
        elif args.command == "load-velocity-models":
            result = app.load_velocity_models(args.store)
        elif args.command == "ensure-demo-survey-time-depth-transform":
            result = app.ensure_demo_survey_time_depth_transform(args.store)
        elif args.command == "import-velocity-functions-model":
            result = app.import_velocity_functions_model(
                args.store,
                args.input,
                velocity_kind=args.velocity_kind
            )
        elif args.command == "prepare-survey-demo":
            result = app.prepare_survey_demo(
                args.store,
                display_coordinate_reference_id=args.display_coordinate_reference_id
            )
        elif args.command == "verify-surface-contracts":
            from .conformance import verify_surface_contracts

            result = verify_surface_contracts()
            print(json.dumps(result, indent=2))
            return 0 if result["ok"] else 1
        else:
            result = app.view_section_horizons(args.store, args.axis, args.index)
    except TraceBoostCommandError as exc:
        if exc.stderr:
            print(exc.stderr, file=sys.stderr)
        print(str(exc), file=sys.stderr)
        return 1

    print(json.dumps(result, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

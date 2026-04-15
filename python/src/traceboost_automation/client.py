from __future__ import annotations

import json
import os
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Sequence


REPO_ROOT = Path(__file__).resolve().parents[3]
DEFAULT_MANIFEST_PATH = REPO_ROOT / "app" / "traceboost-app" / "Cargo.toml"


class TraceBoostCommandError(RuntimeError):
    def __init__(self, message: str, *, command: Sequence[str], stderr: str | None = None) -> None:
        super().__init__(message)
        self.command = list(command)
        self.stderr = stderr


@dataclass(frozen=True)
class TraceBoostApp:
    repo_root: Path = REPO_ROOT
    manifest_path: Path = DEFAULT_MANIFEST_PATH
    binary: str | None = None

    def command_prefix(self) -> list[str]:
        binary = self.binary or os.environ.get("TRACEBOOST_APP_BIN")
        if binary:
            return [binary]
        return [
            "cargo",
            "run",
            "--quiet",
            "--manifest-path",
            str(self.manifest_path),
            "--"
        ]

    def run_json(self, *args: str) -> Any:
        command = [*self.command_prefix(), *args]
        completed = subprocess.run(
            command,
            cwd=self.repo_root,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True
        )
        if completed.returncode != 0:
            raise TraceBoostCommandError(
                f"traceboost-app exited with status {completed.returncode}",
                command=command,
                stderr=completed.stderr.strip() or None
            )
        stdout = completed.stdout.strip()
        if not stdout:
            return None
        try:
            return json.loads(stdout)
        except json.JSONDecodeError as exc:
            raise TraceBoostCommandError(
                "traceboost-app did not return valid JSON",
                command=command,
                stderr=completed.stderr.strip() or stdout
            ) from exc

    def backend_info(self) -> Any:
        return self.run_json("backend-info")

    def operation_catalog(self) -> Any:
        return self.run_json("operation-catalog")

    def preflight_import(
        self,
        input_path: str,
        *,
        inline_byte: int | None = None,
        crossline_byte: int | None = None,
        third_axis_byte: int | None = None,
        inline_type: str = "i32",
        crossline_type: str = "i32",
        third_axis_type: str = "i32"
    ) -> Any:
        args = [
            "preflight-import",
            input_path,
            *header_args("inline", inline_byte, inline_type),
            *header_args("crossline", crossline_byte, crossline_type),
            *header_args("third-axis", third_axis_byte, third_axis_type)
        ]
        return self.run_json(*args)

    def import_dataset(
        self,
        input_path: str,
        output_store_path: str,
        *,
        overwrite_existing: bool = False,
        inline_byte: int | None = None,
        crossline_byte: int | None = None,
        third_axis_byte: int | None = None,
        inline_type: str = "i32",
        crossline_type: str = "i32",
        third_axis_type: str = "i32"
    ) -> Any:
        args = [
            "import-dataset",
            input_path,
            output_store_path,
            *header_args("inline", inline_byte, inline_type),
            *header_args("crossline", crossline_byte, crossline_type),
            *header_args("third-axis", third_axis_byte, third_axis_type)
        ]
        if overwrite_existing:
            args.append("--overwrite-existing")
        return self.run_json(*args)

    def open_dataset(self, store_path: str) -> Any:
        return self.run_json("open-dataset", store_path)

    def set_native_coordinate_reference(
        self,
        store_path: str,
        *,
        coordinate_reference_id: str | None = None,
        coordinate_reference_name: str | None = None
    ) -> Any:
        args = ["set-native-coordinate-reference", store_path]
        if coordinate_reference_id:
            args.extend(["--coordinate-reference-id", coordinate_reference_id])
        if coordinate_reference_name:
            args.extend(["--coordinate-reference-name", coordinate_reference_name])
        return self.run_json(*args)

    def resolve_survey_map(
        self,
        store_path: str,
        *,
        display_coordinate_reference_id: str | None = None
    ) -> Any:
        args = ["resolve-survey-map", store_path]
        if display_coordinate_reference_id:
            args.extend(["--display-coordinate-reference-id", display_coordinate_reference_id])
        return self.run_json(*args)

    def export_segy(self, store_path: str, output_path: str, *, overwrite_existing: bool = False) -> Any:
        args = ["export-segy", store_path, output_path]
        if overwrite_existing:
            args.append("--overwrite-existing")
        return self.run_json(*args)

    def export_zarr(self, store_path: str, output_path: str, *, overwrite_existing: bool = False) -> Any:
        args = ["export-zarr", store_path, output_path]
        if overwrite_existing:
            args.append("--overwrite-existing")
        return self.run_json(*args)

    def import_horizons(
        self,
        store_path: str,
        input_paths: Sequence[str],
        *,
        source_coordinate_reference_id: str | None = None,
        source_coordinate_reference_name: str | None = None,
        assume_same_as_survey: bool = False
    ) -> Any:
        args = ["import-horizons", store_path]
        if source_coordinate_reference_id:
            args.extend(["--source-coordinate-reference-id", source_coordinate_reference_id])
        if source_coordinate_reference_name:
            args.extend(["--source-coordinate-reference-name", source_coordinate_reference_name])
        if assume_same_as_survey:
            args.append("--assume-same-as-survey")
        args.extend(input_paths)
        return self.run_json(*args)

    def view_section(self, store_path: str, axis: str, index: int) -> Any:
        return self.run_json("view-section", store_path, axis, str(index))

    def view_section_horizons(self, store_path: str, axis: str, index: int) -> Any:
        return self.run_json("view-section-horizons", store_path, axis, str(index))

    def load_velocity_models(self, store_path: str) -> Any:
        return self.run_json("load-velocity-models", store_path)

    def ensure_demo_survey_time_depth_transform(self, store_path: str) -> Any:
        return self.run_json("ensure-demo-survey-time-depth-transform", store_path)

    def import_velocity_functions_model(
        self,
        store_path: str,
        input_path: str,
        *,
        velocity_kind: str = "interval"
    ) -> Any:
        return self.run_json(
            "import-velocity-functions-model",
            store_path,
            input_path,
            "--velocity-kind",
            velocity_kind
        )

    def prepare_survey_demo(
        self,
        store_path: str,
        *,
        display_coordinate_reference_id: str | None = None
    ) -> dict[str, Any]:
        args = ["prepare-survey-demo", store_path]
        if display_coordinate_reference_id:
            args.extend(["--display-coordinate-reference-id", display_coordinate_reference_id])
        return self.run_json(*args)


def header_args(name: str, byte: int | None, value_type: str) -> list[str]:
    if byte is None:
        return []
    return [f"--{name}-byte", str(byte), f"--{name}-type", value_type]

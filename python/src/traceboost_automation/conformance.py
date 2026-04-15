from __future__ import annotations

import argparse
from typing import Any

from .catalog import load_operation_catalog
from .cli import build_parser
from .client import TraceBoostApp


INTERNAL_PYTHON_METHODS = {"command_prefix", "run_json"}
INTERNAL_PYTHON_CLI_COMMANDS = {"verify-surface-contracts"}


def verify_surface_contracts() -> dict[str, Any]:
    catalog = load_operation_catalog()
    operations = catalog["operations"]

    declared_python_methods = set()
    declared_python_cli_commands = set()
    issues: list[str] = []

    for operation in operations:
        operation_id = operation["id"]
        bindings = operation.get("bindings", {})
        surfaces = set(operation.get("surfaces", []))

        if "python-api" in surfaces:
            method_name = bindings.get("python_method")
            if not method_name:
                issues.append(f"{operation_id}: python-api surface missing python_method binding")
            else:
                declared_python_methods.add(method_name)

        if "python-cli" in surfaces:
            command_name = bindings.get("python_cli")
            if not command_name:
                issues.append(f"{operation_id}: python-cli surface missing python_cli binding")
            else:
                declared_python_cli_commands.add(command_name)

    actual_python_methods = {
        name
        for name, value in TraceBoostApp.__dict__.items()
        if callable(value) and not name.startswith("_") and name not in INTERNAL_PYTHON_METHODS
    }
    actual_python_cli_commands = parser_subcommand_names(build_parser()) - INTERNAL_PYTHON_CLI_COMMANDS

    for missing_method in sorted(declared_python_methods - actual_python_methods):
        issues.append(f"catalog declares python method '{missing_method}' but TraceBoostApp does not expose it")

    for extra_method in sorted(actual_python_methods - declared_python_methods):
        issues.append(f"TraceBoostApp exposes uncatalogued python method '{extra_method}'")

    for missing_command in sorted(declared_python_cli_commands - actual_python_cli_commands):
        issues.append(
            f"catalog declares python CLI command '{missing_command}' but traceboost-automation does not expose it"
        )

    for extra_command in sorted(actual_python_cli_commands - declared_python_cli_commands):
        issues.append(f"traceboost-automation exposes uncatalogued CLI command '{extra_command}'")

    return {
        "ok": not issues,
        "catalog_name": catalog["catalog_name"],
        "catalog_schema_version": catalog["schema_version"],
        "checked_python_method_count": len(declared_python_methods),
        "checked_python_cli_command_count": len(declared_python_cli_commands),
        "issues": issues
    }


def parser_subcommand_names(parser: argparse.ArgumentParser) -> set[str]:
    for action in parser._actions:
        if isinstance(action, argparse._SubParsersAction):
            return set(action.choices.keys())
    return set()

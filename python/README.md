# traceboost-automation

`traceboost-automation` is a thin Python wrapper around the local `traceboost-app` CLI.

It does not introduce a second backend API. It shells out to the existing JSON-producing app
commands so notebooks, scripts, and local batch jobs can reuse the same workflow surface as the
desktop shell.

## Scope

Current wrapped workflows:

- `operation-catalog`
- `preflight-import`
- `import-dataset`
- `open-dataset`
- `set-native-coordinate-reference`
- `resolve-survey-map`
- `export-segy`
- `export-zarr`
- `import-horizons`
- `view-section`
- `view-section-horizons`
- `load-velocity-models`
- `ensure-demo-survey-time-depth-transform`
- `import-velocity-functions-model`
- `prepare-survey-demo`

## Usage

From this directory:

```bash
python -m pip install -e .
traceboost-automation backend-info
```

Or from Python:

```python
from traceboost_automation import TraceBoostApp

app = TraceBoostApp()
preflight = app.preflight_import("/data/input.segy")
print(preflight["recommended_action"])
```

For a product-shaped local workflow:

```bash
traceboost-automation prepare-survey-demo /data/example.tbvol
```

That command keeps the automation surface thin. It calls the shared Rust-owned TraceBoost workflow service to:

- ensure a demo survey time-depth transform exists
- load the available velocity models
- resolve the survey map payload for chart embedding

If you already have a built binary, set `TRACEBOOST_APP_BIN` to avoid `cargo run` on each call.

## Surface Conformance

TraceBoost now keeps a checked-in operation catalog at
`app/traceboost-app/operations/catalog.json`.

That catalog is the declared inventory for user-facing automation operations. It is used to:

- expose `traceboost-app operation-catalog`
- expose `traceboost-automation operation-catalog`
- verify that the Python wrapper and Python CLI still match the declared automation surface

Run the local Python-side conformance check with:

```bash
traceboost-automation verify-surface-contracts
```

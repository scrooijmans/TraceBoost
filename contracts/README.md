# contracts

`contracts/` contains the shared contracts layer for the TraceBoost monorepo.

This area owns:

- dataset and geometry contracts
- section and tile request/response contracts
- processing parameter contracts
- job and progress event schemas
- IPC-safe interop types that cross app/runtime boundaries

Current crates:

- `seis-contracts-core`
- `seis-contracts-views`
- `seis-contracts-interop`

Frontend-facing generated artifact:

- `ts/seis-contracts`

Regenerate it from the repo root with:

```powershell
.\scripts\generate-ts-contracts.ps1
```

This layer must not own:

- SEG-Y parsing
- chunk caching
- processing kernels
- product workflow logic

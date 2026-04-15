# Multi-Format IO And Runtime Store Strategy

## Context

TraceBoost and Ophiolite already make an important architectural distinction:

- external interchange and ecosystem formats such as `SEG-Y`, `SU`, `Zarr`, `OpenVDS`, `OpenZGY`, and `SGZ`
- internal runtime stores optimized for local processing and interactive visualization

The current runtime is not one store. It is already a family:

- `tbvol` for post-stack regularized volumes
- `tbgath` for prestack offset gathers

That split is a strength, not technical debt. The current benchmarks already show that the hot path is dominated by physical access shape more than by format branding.

## Current read

The current `tbvol` design remains well aligned with the workloads demonstrated in TraceBoost, Ophiolite Charts, and Ophiolite:

- logical shape `[iline, xline, sample]`
- payload type `f32`
- little-endian payload
- tiles that span the full sample axis
- memory-mapped dense amplitude storage
- optional occupancy sidecar for sparse regularized bins

This is especially well suited to:

- inline and xline preview
- trace-local operators
- bounded-working-set full-volume materialization
- deterministic derived-store writes

The key implementation constraint is explicit in the runtime: `tbvol` tiles must span the full sample axis. That is the reason the store performs well for operators such as RMS normalization, phase rotation, and bandpass variants.

## Recommendation

Do not replace `tbvol` with a generic ND chunked container.

Instead:

- keep `tbvol` as the default hot path for post-stack runtime compute
- keep `tbgath` as the prestack runtime path
- treat `MDIO/Zarr`, `OpenVDS`, `OpenZGY`, `SGZ`, and `HDF5` as import/export or transcode adapters first
- evolve metadata and format-adapter boundaries before evolving the `tbvol` payload layout

The main conclusion is:

- `tbvol v2` should generalize metadata and provenance
- `tbvol v2` should not become a generic cloud-native chunk store

## Why MDIO/Zarr and OpenVDS still matter

These formats are still important, just for different reasons.

### MDIO / Zarr

MDIO is attractive because it standardizes chunked multidimensional seismic on top of Zarr and explicitly supports `2D` to `5D` SEG-Y import and export. It also has stronger xarray, Dask, and object-store ergonomics than `tbvol`.

This makes it a strong candidate for:

- external import and export
- cloud and data-science workflows
- interchange with Python ecosystems
- archival and collaboration workflows

It does not by itself prove that a local desktop hot path should become generic ND chunking.

### OpenVDS

OpenVDS is attractive because it explicitly models seismic semantics, dimensions, channels, and survey coordinate metadata, while supporting random access and up to `6D` volumes.

This makes it a strong candidate for:

- interoperability
- local and cloud volumetric exchange
- large random-access browsing
- survey-coordinate-rich format adapters

OpenVDS metadata is also a useful model for how TraceBoost should enrich its canonical metadata surface even if it keeps a simpler runtime payload.

## What should change in `tbvol`

### Keep unchanged

- dense `f32` processing payload for the hot path
- full-sample-axis tile rule for post-stack compute
- memory-mapped local-file access
- derived-store lineage and immutable materialization

### Change in metadata

Add a richer canonical descriptor for runtime stores and imported datasets:

- explicit dimension roles rather than implicit `iline/xline/sample` assumptions
- axis names and units
- logical coordinates and physical coordinates as separate concepts
- CRS and survey-grid transform metadata
- optional auxiliary coordinate variables
- source-format provenance packages
- explicit trace-order and source-layout provenance
- support for extended SEG-Y export metadata without breaking older manifests

### Add optional sidecars

Optional sidecars are likely worthwhile without changing the base amplitude payload:

- multiresolution overviews for visualization
- thumbnail or coarse section caches
- horizon or interpretation attachments
- source-format export packages

These should remain optional sidecars, not mandatory complexity in the base store.

## What should not change yet

### 2D

Do not invent a separate post-stack `2D` runtime format now.

The better default is:

- represent post-stack `2D` as a degenerate `tbvol`
- allow one spatial axis to be length `1`
- preserve explicit layout metadata so the UI and downstream tools know this is `PostStack2D`

### 4D

Do not move immediately to one monolithic four-axis runtime payload.

The better first step is:

- store each vintage as its own aligned `tbvol`
- add relationship metadata describing vintage membership, alignment, and comparison assumptions
- materialize derived 4D products as either sibling stores or explicit cross-vintage products

This keeps current 3D preview and apply performance intact while still supporting time-lapse workflows.

### Prestack

Do not force prestack into `tbvol`.

The current split is directionally correct:

- `tbvol` for post-stack section-centric processing
- `tbgath` for gather-native prestack access

If future prestack workloads need another optimized store, add a new specialized runtime store instead of flattening everything into one generic container.

## Fixture-based implications from current test data

The current repository already contains a useful correctness corpus:

- post-stack 3D: `f3.sgy`, `small.sgy`, geometry variants
- little-endian SEG-Y: `small-lsb.sgy`, `f3-lsb.sgy`
- extended textual headers: `multi-text.sgy`
- degenerate 2D-like shapes: `1xN.sgy`, `Mx1.sgy`, `1x1.sgy`
- prestack small fixtures: `small-ps.sgy` and sorting variants
- unstructured gathers and odd cases: `shot-gather.sgy`, `long.sgy`, `text.sgy`
- SU fixtures: `small.su`, `small-lsb.su`
- existing Zarr fixture: `test-data/survey.zarr`

Using `segyio` on the local fixture set confirms:

- `1xN.sgy` and `Mx1.sgy` behave like degenerate regular grids rather than requiring a new runtime payload
- `small-ps*.sgy` fixtures expose a real third gather axis and should stay on the `tbgath` path
- `multi-text.sgy` proves that source-format provenance capture matters
- `small-lsb.sgy` and `f3-lsb.sgy` require explicit source endianness handling
- `shot-gather.sgy` and `long.sgy` are useful stress cases for unstructured and long-trace ingest

## Benchmark matrix to add next

The existing compute benchmark already covers post-stack local runtime candidates. The next benchmark wave should cover format strategy rather than only local throughput.

### Tier 1: correctness and metadata roundtrip

- `SEG-Y -> tbvol -> SEG-Y -> tbvol`
- `SEG-Y -> tbvol -> MDIO/Zarr -> tbvol`
- `SEG-Y -> tbvol -> OpenVDS -> tbvol`
- `SEG-Y -> tbvol -> OpenZGY -> tbvol` when a practical writer path is available

Acceptance bar:

- same amplitudes after re-import
- same relevant header metadata after re-import
- preserved textual and binary headers where the target format supports that concept
- preserved logical layout, axis units, and CRS metadata

### Tier 2: post-stack runtime performance

- `f3.sgy`
- `small.sgy`
- synthetic `256 x 256 x 1024`
- synthetic large survey matching the existing benchmark scale

Compare:

- `tbvol`
- current `Zarr`
- external `OpenVDS`
- external `SGZ`

### Tier 3: degenerate 2D and line data

- `1xN.sgy`
- `Mx1.sgy`
- synthetic long-line datasets

Questions to answer:

- does a degenerate `tbvol` remain performant enough
- is preview latency still dominated by section assembly rather than tile shape
- do we need a dedicated line-store or only layout-aware metadata

### Tier 4: prestack

- `small-ps.sgy`
- sorting variants under `small-ps-dec-*`
- `shot-gather.sgy`

Questions to answer:

- is `tbgath` still the right hot path
- do we need sparse prestack regularization support
- which external formats preserve gather semantics cleanly enough for roundtrip and import

### Tier 5: synthetic 4D

Create aligned synthetic vintages:

- baseline `3D`
- monitor `3D`
- delta product

Questions to answer:

- is a multi-store vintage family sufficient
- which operations require cross-vintage co-read
- when would a dedicated 4D runtime store outperform aligned sibling `tbvol`s

## Licensing read

This is an engineering summary, not legal advice.

- `MDIO`: Apache-2.0
- `Zarr`: MIT
- `OpenVDS`: Apache-2.0
- `pyzgy` and bundled `openzgy` path described by the repo: Apache-2.0
- `HDF5`: permissive HDF5 license allowing commercial use
- `seismic-zfp`: LGPL-3.0

That implies:

- `MDIO/Zarr`, `OpenVDS`, `OpenZGY`, and `HDF5` are acceptable candidates for commercial adapter work
- `seismic-zfp` is still useful technically, but it has higher distribution and linkage friction for a commercial desktop product

## Canonical adapter model

The safest long-term shape is:

`external format <-> canonical dataset model <-> specialized runtime store`

The canonical dataset model should carry:

- dimensions and dimension roles
- coordinate variables and units
- CRS and survey-grid transforms
- source-format provenance
- optional per-trace or per-voxel auxiliary fields
- export packages for lossless roundtrip when possible

Then map that model onto:

- `tbvol` for post-stack runtime compute
- `tbgath` for prestack runtime compute
- future specialized stores only when a benchmark proves a real workload mismatch

## Practical next steps

1. Add `tbvol v2` descriptor metadata without changing the amplitude payload layout.
2. Add a vintage-family metadata model instead of a monolithic 4D store.
3. Keep `PostStack2D` on the `tbvol` path and benchmark it explicitly.
4. Keep `tbgath` as the prestack path and expand sparse prestack handling only if benchmarks justify it.
5. Add an explicit format-adapter boundary in Ophiolite IO.
6. Implement `MDIO/Zarr` import and export first.
7. Implement `OpenVDS` import next, then export if the roundtrip semantics are acceptable.
8. Treat `OpenZGY` and `HDF5` as secondary adapters after `MDIO/Zarr` and `OpenVDS`.
9. Keep `SGZ` in the benchmark set, but do not make it a shipped core dependency decision until licensing is cleared for the product shape.

## References

- Local benchmark note: `articles/storage/SEISMIC_VOLUME_STORAGE_AND_BENCHMARKING.md`
- OpenVDS metadata spec: <https://osdu.pages.opengroup.org/platform/domain-data-mgmt-services/seismic/open-vds/vds/specification/Metadata.html>
- OpenVDS repository: <https://github.com/wadesalazar/open-vds>
- MDIO repository: <https://github.com/TGSAI/mdio-python>
- MDIO documentation PDF: <https://mdio-python.readthedocs.io/_/downloads/en/v0.9.3/pdf/>
- Zarr repository: <https://github.com/zarr-developers/zarr-python>
- pyzgy repository: <https://github.com/equinor/pyzgy>
- h5geo documentation PDF: <https://h5geo.readthedocs.io/_/downloads/en/latest/pdf/>
- HDF5 license terms: <https://support.hdfgroup.org/ftp/HDF5/releases/COPYING.html>
- seismic-zfp repository: <https://github.com/equinor/seismic-zfp>

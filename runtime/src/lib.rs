mod error;
mod ingest;
mod metadata;
mod preflight;
mod render;
mod store;
mod upscale;
mod validation;

pub use error::SeisRefineError;
pub use ophiolite_seismic_runtime::{
    MaterializeOptions, apply_pipeline_to_plane, apply_pipeline_to_traces,
    materialize_from_reader_writer, materialize_volume, preview_section_from_reader,
    preview_section_plane, preview_section_view, validate_pipeline,
};
pub use ingest::{
    IngestOptions, SeisGeometryOptions, SourceVolume, SparseSurveyPolicy, ingest_segy,
    load_source_volume, load_source_volume_with_options, recommended_chunk_shape,
};
pub use metadata::{
    DatasetKind, GeometryProvenance, HeaderFieldSpec, InterpMethod, ProcessingLineage,
    ProcessingOperation, RegularizationProvenance, SourceIdentity, TbvolManifest, VolumeAxes,
    VolumeMetadata,
};
pub use preflight::{PreflightAction, PreflightGeometry, SurveyPreflight, preflight_segy};
pub use render::{render_section_csv, render_section_csv_for_request};
pub use ophiolite_seismic::{
    DatasetId, InterpretationPoint, ProcessingParameters, SectionAxis, SectionRequest,
    SectionTileRequest, VolumeDescriptor,
};
pub use ophiolite_seismic::{PreviewView, SectionView};
pub use ophiolite_seismic_runtime::{
    OccupancyTile, TbvolReader, TbvolWriter, TileBuffer, TileCoord, TileGeometry,
    VolumeStoreReader, VolumeStoreWriter, assemble_section_plane, recommended_tbvol_tile_shape,
};
pub use store::{
    SectionPlane, StoreHandle, create_tbvol_store, describe_store, load_array, load_occupancy,
    open_store, read_section_plane, section_view,
};
pub use upscale::{UpscaleOptions, upscale_2x, upscale_cubic_2x, upscale_linear_2x, upscale_store};
pub use validation::{
    ValidationDatasetReport, ValidationMethodReport, ValidationMetrics, ValidationOptions,
    ValidationSummary, run_validation, validate_dataset,
};

pub use ophiolite_seismic_runtime::{SegyInspection, inspect_segy};

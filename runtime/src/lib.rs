mod error;
mod ingest;
mod metadata;
mod preflight;
mod render;
mod store;
mod upscale;
mod validation;

pub use error::SeisRefineError;
pub use ingest::{
    IngestOptions, SeisGeometryOptions, SourceVolume, SparseSurveyPolicy, ingest_segy,
    load_source_volume, load_source_volume_with_options, recommended_chunk_shape,
};
pub use metadata::{
    DatasetKind, GeometryProvenance, HeaderFieldSpec, InterpMethod, ProcessingLineage,
    RegularizationProvenance, SourceIdentity, TbvolManifest, VolumeAxes, VolumeMetadata,
};
pub use ophiolite_seismic::{
    AmplitudeSpectrumCurve, AmplitudeSpectrumRequest, AmplitudeSpectrumResponse, AxisSummaryF32,
    AxisSummaryI32, CancelProcessingJobRequest, CancelProcessingJobResponse, DatasetId,
    DeletePipelinePresetRequest, DeletePipelinePresetResponse, FrequencyPhaseMode,
    FrequencyWindowShape, GatherInterpolationMode, GatherPreviewView, GatherProcessingOperation,
    GatherProcessingPipeline, GatherRequest, GatherSelector, GeometryDescriptor,
    GeometryProvenanceSummary, GeometrySummary, GetProcessingJobRequest, GetProcessingJobResponse,
    ImportHorizonXyzRequest, ImportHorizonXyzResponse, ImportPrestackOffsetDatasetRequest,
    ImportPrestackOffsetDatasetResponse, ImportedHorizonDescriptor, InterpretationPoint,
    ListPipelinePresetsResponse, LoadSectionHorizonsRequest, LoadSectionHorizonsResponse,
    PrestackThirdAxisField, PreviewGatherProcessingRequest, PreviewGatherProcessingResponse,
    PreviewProcessingRequest, PreviewProcessingResponse, PreviewTraceLocalProcessingRequest,
    PreviewTraceLocalProcessingResponse, ProcessingArtifactRole, ProcessingJobArtifact,
    ProcessingJobArtifactKind, ProcessingJobProgress, ProcessingJobState, ProcessingJobStatus,
    ProcessingOperation, ProcessingPipeline, ProcessingPipelineFamily, ProcessingPipelineSpec,
    ProcessingPreset, RunGatherProcessingRequest, RunGatherProcessingResponse,
    RunProcessingRequest, RunProcessingResponse, RunTraceLocalProcessingRequest,
    RunTraceLocalProcessingResponse, SavePipelinePresetRequest, SavePipelinePresetResponse,
    SectionAxis, SectionHorizonLineStyle, SectionHorizonOverlayView, SectionHorizonSample,
    SectionHorizonStyle, SectionRequest, SectionSpectrumSelection, SectionTileRequest,
    SemblancePanel, SubvolumeProcessingPipeline, TraceLocalProcessingOperation,
    TraceLocalProcessingPipeline, TraceLocalProcessingPreset, TraceLocalProcessingStep,
    TraceLocalVolumeArithmeticOperator, VelocityAutopickParameters, VelocityFunctionEstimate,
    VelocityFunctionSource, VelocityPickStrategy, VelocityScanRequest, VelocityScanResponse,
    VolumeDescriptor,
};
pub use ophiolite_seismic::{PreviewView, SectionView};
pub use ophiolite_seismic_runtime::{
    MaterializeOptions, PreviewSectionPrefixCache, PreviewSectionPrefixReuse,
    PreviewSectionSession, SeismicStoreError, amplitude_spectrum_from_plane,
    amplitude_spectrum_from_reader, amplitude_spectrum_from_store, apply_pipeline_to_plane,
    apply_pipeline_to_traces, export_store_to_segy, materialize_from_reader_writer,
    materialize_from_reader_writer_with_progress, materialize_processing_volume,
    materialize_processing_volume_with_progress, materialize_subvolume_processing_volume,
    materialize_subvolume_processing_volume_with_progress, materialize_volume,
    preview_processing_section_plane, preview_processing_section_view,
    preview_processing_section_view_with_prefix_cache, preview_section_from_reader,
    preview_section_plane, preview_section_view, preview_section_view_with_prefix_cache,
    preview_subvolume_processing_section_view, validate_pipeline, validate_processing_pipeline,
    velocity_scan,
};
pub use ophiolite_seismic_runtime::{
    OccupancyTile, PrestackStoreHandle, TbgathManifest, TbgathReader, TbgathWriter, TbvolReader,
    TbvolWriter, TileBuffer, TileCoord, TileGeometry, VolumeStoreReader, VolumeStoreWriter,
    assemble_section_plane, create_tbgath_store, describe_prestack_store,
    ingest_prestack_offset_segy, open_prestack_store, prestack_gather_view,
    preview_gather_processing_view, read_prestack_gather_plane,
    recommended_default_tbvol_tile_target_mib, recommended_tbvol_tile_shape,
    set_any_store_native_coordinate_reference,
};
pub use ophiolite_seismic_runtime::{
    materialize_gather_processing_store, materialize_gather_processing_store_with_progress,
};
pub use preflight::{PreflightAction, PreflightGeometry, SurveyPreflight, preflight_segy};
pub use render::{render_section_csv, render_section_csv_for_request};
pub use store::{
    SectionPlane, StoreHandle, create_tbvol_store, describe_store, import_horizon_xyzs, load_array,
    load_occupancy, open_store, read_section_plane, section_horizon_overlays, section_view,
};
pub use upscale::{UpscaleOptions, upscale_2x, upscale_cubic_2x, upscale_linear_2x, upscale_store};
pub use validation::{
    ValidationDatasetReport, ValidationMethodReport, ValidationMetrics, ValidationOptions,
    ValidationSummary, run_validation, validate_dataset,
};

pub use ophiolite_seismic_runtime::{SegyInspection, inspect_segy};

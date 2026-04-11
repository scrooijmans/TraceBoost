use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use seis_contracts_interop::{
    AmplitudeSpectrumRequest, AmplitudeSpectrumResponse, DatasetSummary, ExportSegyRequest,
    ExportSegyResponse, GatherProcessingPipeline, GatherRequest, GatherView, IPC_SCHEMA_VERSION,
    ImportDatasetRequest, ImportDatasetResponse, ImportHorizonXyzRequest, ImportHorizonXyzResponse,
    ImportPrestackOffsetDatasetRequest, ImportPrestackOffsetDatasetResponse,
    LoadSectionHorizonsRequest, LoadSectionHorizonsResponse, LoadVelocityModelsResponse,
    OpenDatasetRequest, OpenDatasetResponse, PrestackThirdAxisField,
    PreviewGatherProcessingRequest, PreviewGatherProcessingResponse,
    PreviewSubvolumeProcessingRequest, PreviewSubvolumeProcessingResponse,
    PreviewTraceLocalProcessingRequest, PreviewTraceLocalProcessingResponse,
    RunGatherProcessingRequest, RunSubvolumeProcessingRequest, RunTraceLocalProcessingRequest,
    SegyGeometryCandidate, SegyGeometryOverride, SegyHeaderField, SegyHeaderValueType,
    SubvolumeProcessingPipeline, SuggestedImportAction, SurveyPreflightRequest,
    SurveyPreflightResponse, VelocityFunctionSource, VelocityScanRequest, VelocityScanResponse,
};
use seis_io::HeaderField;
use seis_runtime::{
    BuildSurveyTimeDepthTransformRequest, DepthReferenceKind, GatherInterpolationMode,
    IngestOptions, LateralInterpolationMethod, LayeredVelocityInterval, LayeredVelocityModel,
    MaterializeOptions, PreviewView, ProjectedPoint2, ResolvedSectionDisplayView,
    SeisGeometryOptions, SparseSurveyPolicy, SpatialCoverageRelationship, SpatialCoverageSummary,
    StratigraphicBoundaryReference, SurveyTimeDepthTransform3D, TimeDepthDomain,
    TimeDepthTransformSourceKind, TraceLocalProcessingPipeline, TravelTimeReference,
    VelocityControlProfile, VelocityControlProfileSample, VelocityControlProfileSet,
    VelocityIntervalTrend, VelocityQuantityKind, VerticalAxisDescriptor,
    VerticalInterpolationMethod, amplitude_spectrum_from_store, build_survey_time_depth_transform,
    depth_converted_section_view, describe_prestack_store, describe_store, export_store_to_segy,
    export_store_to_zarr, import_horizon_xyzs, ingest_prestack_offset_segy, ingest_volume,
    load_survey_time_depth_transforms, materialize_gather_processing_store,
    materialize_processing_volume, materialize_subvolume_processing_volume, open_prestack_store,
    open_store, preflight_segy, prestack_gather_view, preview_gather_processing_view,
    preview_processing_section_view, preview_subvolume_processing_section_view,
    resolved_section_display_view, section_horizon_overlays, store_survey_time_depth_transform,
    velocity_scan,
};
use serde::Serialize;

const DEFAULT_SPARSE_FILL_VALUE: f32 = 0.0;
const DEMO_SURVEY_TIME_DEPTH_TRANSFORM_ID: &str = "demo-survey-3d-transform";
const DEMO_SURVEY_TIME_DEPTH_TRANSFORM_NAME: &str = "Synthetic Survey 3D Time-Depth Transform";

#[derive(Debug, Clone, Serialize)]
pub struct ExportZarrResponse {
    pub store_path: String,
    pub output_path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportVelocityFunctionsModelResponse {
    pub schema_version: u32,
    pub input_path: String,
    pub velocity_kind: VelocityQuantityKind,
    pub profile_count: usize,
    pub sample_count: usize,
    pub model: SurveyTimeDepthTransform3D,
}

#[derive(Debug, Clone)]
struct ParsedVelocityFunctions {
    profiles: Vec<VelocityControlProfile>,
    sample_count: usize,
}

#[derive(Debug, Clone)]
struct ParsedVelocityProfileRow {
    x: f64,
    y: f64,
    sample: VelocityControlProfileSample,
}

#[derive(Debug, Clone)]
struct GeometryCandidateSpec {
    label: &'static str,
    inline: (u16, SegyHeaderValueType),
    crossline: (u16, SegyHeaderValueType),
}

const GEOMETRY_CANDIDATE_SPECS: [GeometryCandidateSpec; 7] = [
    GeometryCandidateSpec {
        label: "Legacy EP / trace-in-record (17/13)",
        inline: (17, SegyHeaderValueType::I32),
        crossline: (13, SegyHeaderValueType::I32),
    },
    GeometryCandidateSpec {
        label: "CDP / trace-in-record (21/13)",
        inline: (21, SegyHeaderValueType::I32),
        crossline: (13, SegyHeaderValueType::I32),
    },
    GeometryCandidateSpec {
        label: "EP / trace-sequence-file (17/9)",
        inline: (17, SegyHeaderValueType::I32),
        crossline: (9, SegyHeaderValueType::I32),
    },
    GeometryCandidateSpec {
        label: "CDP / trace-sequence-file (21/9)",
        inline: (21, SegyHeaderValueType::I32),
        crossline: (9, SegyHeaderValueType::I32),
    },
    GeometryCandidateSpec {
        label: "Trace-sequence-file / trace-in-record (9/13)",
        inline: (9, SegyHeaderValueType::I32),
        crossline: (13, SegyHeaderValueType::I32),
    },
    GeometryCandidateSpec {
        label: "Trace-sequence-line / trace-in-record (1/13)",
        inline: (1, SegyHeaderValueType::I32),
        crossline: (13, SegyHeaderValueType::I32),
    },
    GeometryCandidateSpec {
        label: "Trace-sequence-line / trace-sequence-file (1/9)",
        inline: (1, SegyHeaderValueType::I32),
        crossline: (9, SegyHeaderValueType::I32),
    },
];

fn materialize_options_for_store(
    input_store_path: &str,
) -> Result<MaterializeOptions, Box<dyn std::error::Error>> {
    let chunk_shape = open_store(input_store_path)?.manifest.tile_shape;
    Ok(MaterializeOptions {
        chunk_shape,
        ..MaterializeOptions::default()
    })
}

pub fn preflight_dataset(
    request: SurveyPreflightRequest,
) -> Result<SurveyPreflightResponse, Box<dyn std::error::Error>> {
    let geometry_override = request.geometry_override.clone();
    let input_path = request.input_path.clone();
    let preflight = preflight_segy(
        &request.input_path,
        &ingest_options_from_geometry_override(geometry_override.as_ref()),
    )?;
    let candidates = if geometry_override.is_none()
        && matches!(
            preflight.recommended_action,
            seis_runtime::PreflightAction::ReviewGeometryMapping
        ) {
        discover_geometry_candidates(&request.input_path, &preflight)
    } else {
        Vec::new()
    };
    let suggested_geometry_override = preferred_geometry_override(&candidates);
    Ok(preflight_response(
        input_path,
        &preflight,
        suggested_geometry_override,
        candidates,
    ))
}

pub fn import_dataset(
    request: ImportDatasetRequest,
) -> Result<ImportDatasetResponse, Box<dyn std::error::Error>> {
    let input = PathBuf::from(&request.input_path);
    let output = PathBuf::from(&request.output_store_path);
    prepare_output_store(&input, &output, request.overwrite_existing)?;
    let handle = ingest_volume(
        &input,
        &output,
        IngestOptions {
            geometry: geometry_override_to_seis_options(request.geometry_override.as_ref()),
            sparse_survey_policy: SparseSurveyPolicy::RegularizeToDense {
                fill_value: DEFAULT_SPARSE_FILL_VALUE,
            },
            ..IngestOptions::default()
        },
    )?;
    Ok(ImportDatasetResponse {
        schema_version: IPC_SCHEMA_VERSION,
        dataset: dataset_summary_for_path(&handle.root)?,
    })
}

pub fn import_prestack_offset_dataset(
    request: ImportPrestackOffsetDatasetRequest,
) -> Result<ImportPrestackOffsetDatasetResponse, Box<dyn std::error::Error>> {
    let input = PathBuf::from(&request.input_path);
    let output = PathBuf::from(&request.output_store_path);
    prepare_output_store(&input, &output, request.overwrite_existing)?;
    let handle = ingest_prestack_offset_segy(
        &input,
        &output,
        IngestOptions {
            geometry: SeisGeometryOptions {
                third_axis_field: Some(prestack_third_axis_field(request.third_axis_field)),
                ..SeisGeometryOptions::default()
            },
            ..IngestOptions::default()
        },
    )?;
    Ok(ImportPrestackOffsetDatasetResponse {
        schema_version: IPC_SCHEMA_VERSION,
        dataset: dataset_summary_for_path(&handle.root)?,
    })
}

fn prepare_output_store(
    input_path: &Path,
    output_path: &Path,
    overwrite_existing: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if !overwrite_existing || !output_path.exists() {
        return Ok(());
    }

    let input_path = input_path
        .canonicalize()
        .unwrap_or_else(|_| input_path.to_path_buf());
    let output_path = output_path
        .canonicalize()
        .unwrap_or_else(|_| output_path.to_path_buf());

    if input_path == output_path {
        return Err("Output store path cannot overwrite the input SEG-Y file.".into());
    }

    let metadata = fs::symlink_metadata(&output_path)?;
    if metadata.file_type().is_dir() {
        fs::remove_dir_all(&output_path)?;
    } else {
        fs::remove_file(&output_path)?;
    }

    Ok(())
}

pub fn open_dataset_summary(
    request: OpenDatasetRequest,
) -> Result<OpenDatasetResponse, Box<dyn std::error::Error>> {
    let store_path = request.store_path;
    Ok(OpenDatasetResponse {
        schema_version: IPC_SCHEMA_VERSION,
        dataset: dataset_summary_for_path(&store_path)?,
    })
}

pub fn export_dataset_segy(
    request: ExportSegyRequest,
) -> Result<ExportSegyResponse, Box<dyn std::error::Error>> {
    let store_path = PathBuf::from(&request.store_path);
    let output_path = PathBuf::from(&request.output_path);
    prepare_export_output_path(
        &store_path,
        &output_path,
        request.overwrite_existing,
        "SEG-Y file",
    )?;
    export_store_to_segy(&store_path, &output_path, request.overwrite_existing)?;
    Ok(ExportSegyResponse {
        schema_version: IPC_SCHEMA_VERSION,
        store_path: request.store_path,
        output_path: request.output_path,
    })
}

pub fn export_dataset_zarr(
    store_path: String,
    output_path: String,
    overwrite_existing: bool,
) -> Result<ExportZarrResponse, Box<dyn std::error::Error>> {
    let store_path_buf = PathBuf::from(&store_path);
    let output_path_buf = PathBuf::from(&output_path);
    prepare_export_output_path(
        &store_path_buf,
        &output_path_buf,
        overwrite_existing,
        "Zarr store",
    )?;
    export_store_to_zarr(&store_path_buf, &output_path_buf, overwrite_existing)?;
    Ok(ExportZarrResponse {
        store_path,
        output_path,
    })
}

pub fn import_horizon_xyz(
    request: ImportHorizonXyzRequest,
) -> Result<ImportHorizonXyzResponse, Box<dyn std::error::Error>> {
    let imported = import_horizon_xyzs(
        &request.store_path,
        &request
            .input_paths
            .iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>(),
        request.source_coordinate_reference_id.as_deref(),
        request.source_coordinate_reference_name.as_deref(),
        request.assume_same_as_survey,
    )?;
    Ok(ImportHorizonXyzResponse {
        schema_version: IPC_SCHEMA_VERSION,
        imported,
    })
}

pub fn load_section_horizons(
    request: LoadSectionHorizonsRequest,
) -> Result<LoadSectionHorizonsResponse, Box<dyn std::error::Error>> {
    let overlays = section_horizon_overlays(&request.store_path, request.axis, request.index)?;
    Ok(LoadSectionHorizonsResponse {
        schema_version: IPC_SCHEMA_VERSION,
        overlays,
    })
}

pub fn load_depth_converted_section(
    store_path: String,
    axis: seis_runtime::SectionAxis,
    index: usize,
    velocity_model: VelocityFunctionSource,
    velocity_kind: seis_runtime::VelocityQuantityKind,
) -> Result<seis_runtime::SectionView, Box<dyn std::error::Error>> {
    let handle = open_store(&store_path)?;
    let section =
        depth_converted_section_view(&store_path, axis, index, &velocity_model, velocity_kind)?;
    ensure_dataset_matches(&handle, &section.dataset_id.0)?;
    Ok(section)
}

pub fn load_resolved_section_display(
    store_path: String,
    axis: seis_runtime::SectionAxis,
    index: usize,
    domain: TimeDepthDomain,
    velocity_model: Option<VelocityFunctionSource>,
    velocity_kind: Option<seis_runtime::VelocityQuantityKind>,
    include_velocity_overlay: bool,
) -> Result<ResolvedSectionDisplayView, Box<dyn std::error::Error>> {
    let handle = open_store(&store_path)?;
    let display = resolved_section_display_view(
        &store_path,
        axis,
        index,
        domain,
        velocity_model.as_ref(),
        velocity_kind,
        include_velocity_overlay,
    )?;
    ensure_dataset_matches(&handle, &display.section.dataset_id.0)?;
    Ok(display)
}

pub fn ensure_demo_survey_time_depth_transform(
    store_path: String,
) -> Result<String, Box<dyn std::error::Error>> {
    let handle = open_store(&store_path)?;
    let sample_axis_ms = &handle.manifest.volume.axes.sample_axis_ms;
    if sample_axis_ms.is_empty() {
        return Err(
            "Cannot create a survey time-depth transform for a store without a sample axis.".into(),
        );
    }

    let shape = handle.manifest.volume.shape;
    let inline_count = shape[0];
    let xline_count = shape[1];
    let sample_count = shape[2];
    if inline_count == 0 || xline_count == 0 || sample_count == 0 {
        return Err("Cannot create a survey time-depth transform for an empty survey grid.".into());
    }

    let time_axis = VerticalAxisDescriptor {
        domain: TimeDepthDomain::Time,
        unit: "ms".to_string(),
        start: sample_axis_ms[0],
        step: inferred_sample_interval_ms(sample_axis_ms),
        count: sample_axis_ms.len(),
    };
    let descriptor = SurveyTimeDepthTransform3D {
        id: DEMO_SURVEY_TIME_DEPTH_TRANSFORM_ID.to_string(),
        name: DEMO_SURVEY_TIME_DEPTH_TRANSFORM_NAME.to_string(),
        derived_from: vec![handle.dataset_id().0.clone()],
        source_kind: TimeDepthTransformSourceKind::VelocityGrid3D,
        coordinate_reference: handle
            .manifest
            .volume
            .coordinate_reference_binding
            .as_ref()
            .and_then(|binding| binding.effective.clone()),
        grid_transform: handle
            .manifest
            .volume
            .spatial
            .as_ref()
            .and_then(|spatial| spatial.grid_transform.clone()),
        time_axis,
        depth_unit: "m".to_string(),
        inline_count,
        xline_count,
        sample_count,
        coverage: SpatialCoverageSummary {
            relationship: SpatialCoverageRelationship::Exact,
            source_coordinate_reference: handle
                .manifest
                .volume
                .coordinate_reference_binding
                .as_ref()
                .and_then(|binding| binding.effective.clone()),
            target_coordinate_reference: handle
                .manifest
                .volume
                .coordinate_reference_binding
                .as_ref()
                .and_then(|binding| binding.effective.clone()),
            notes: vec![
                "Synthetic survey-aligned trace-varying transform for time-depth demo workflows."
                    .to_string(),
            ],
        },
        notes: vec![
            "This transform is synthetic demo data, not an imported velocity model.".to_string(),
            "It is survey-aligned and spatially varying so TraceBoost can exercise the survey-3D section conversion path.".to_string(),
        ],
    };

    let cell_count = inline_count * xline_count * sample_count;
    let mut depths_m = vec![0.0_f32; cell_count];
    let validity = vec![1_u8; cell_count];
    for inline_index in 0..inline_count {
        for xline_index in 0..xline_count {
            let mut cumulative_depth_m = 0.0_f32;
            let inline_ratio = normalized_index(inline_index, inline_count);
            let xline_ratio = normalized_index(xline_index, xline_count);
            let structural_uplift =
                (-(distance_squared(inline_ratio, xline_ratio, 0.58, 0.46) / 0.035)).exp() * 14.0;
            let layer_one = 0.18 + f32::sin(inline_ratio * std::f32::consts::TAU * 1.15) * 0.035;
            let layer_two =
                0.36 + f32::sin(xline_ratio * std::f32::consts::TAU * 1.35 + 0.55) * 0.045;
            let layer_three =
                0.56 + f32::sin((inline_ratio + xline_ratio) * std::f32::consts::PI * 1.4) * 0.05;
            let layer_four = 0.74
                + f32::cos((inline_ratio * 0.7 + xline_ratio * 1.3) * std::f32::consts::PI * 1.6)
                    * 0.055;

            let mut previous_time_ms = 0.0_f32;
            for sample_index in 0..sample_count {
                let offset =
                    ((inline_index * xline_count + xline_index) * sample_count) + sample_index;
                let time_ms = sample_axis_ms[sample_index];
                let dt_ms = if sample_index == 0 {
                    time_ms.max(0.0)
                } else {
                    (time_ms - previous_time_ms).max(0.0)
                };
                previous_time_ms = time_ms;

                let vertical_ratio = normalized_index(sample_index, sample_count)
                    - structural_uplift / sample_count as f32;
                let layer_index = if vertical_ratio < layer_one {
                    0
                } else if vertical_ratio < layer_two {
                    1
                } else if vertical_ratio < layer_three {
                    2
                } else if vertical_ratio < layer_four {
                    3
                } else {
                    4
                };
                let base_velocity_m_per_s =
                    [1525.0_f32, 1810.0, 2225.0, 2735.0, 3320.0][layer_index];
                let lateral_trend = f32::sin(inline_ratio * std::f32::consts::TAU * 1.3) * 130.0
                    + f32::cos(xline_ratio * std::f32::consts::TAU * 1.1) * 95.0;
                let local_variation = f32::sin(sample_index as f32 / 17.0 + inline_ratio * 4.8)
                    * 36.0
                    + f32::cos(sample_index as f32 / 23.0 + xline_ratio * 5.6) * 28.0;
                let deepening_trend = normalized_index(sample_index, sample_count) * 260.0;
                let interval_velocity_m_per_s =
                    (base_velocity_m_per_s + lateral_trend + local_variation + deepening_trend)
                        .clamp(1450.0, 3900.0);

                cumulative_depth_m += interval_velocity_m_per_s * (dt_ms * 0.001) * 0.5;
                depths_m[offset] = cumulative_depth_m;
            }
        }
    }

    let stored = store_survey_time_depth_transform(&store_path, descriptor, &depths_m, &validity)?;
    Ok(stored.id)
}

pub fn load_velocity_models(
    store_path: String,
) -> Result<LoadVelocityModelsResponse, Box<dyn std::error::Error>> {
    let models = load_survey_time_depth_transforms(&store_path)?
        .into_iter()
        .map(|transform| transform.descriptor)
        .collect::<Vec<_>>();
    Ok(LoadVelocityModelsResponse {
        schema_version: IPC_SCHEMA_VERSION,
        models,
    })
}

pub fn build_velocity_model_transform(
    request: BuildSurveyTimeDepthTransformRequest,
) -> Result<SurveyTimeDepthTransform3D, Box<dyn std::error::Error>> {
    let model = build_survey_time_depth_transform(&request)?;
    Ok(model)
}

pub fn import_velocity_functions_model(
    store_path: String,
    input_path: String,
    velocity_kind: VelocityQuantityKind,
) -> Result<ImportVelocityFunctionsModelResponse, Box<dyn std::error::Error>> {
    if matches!(velocity_kind, VelocityQuantityKind::Rms) {
        return Err(
            "Velocity_functions.txt import currently supports interval or average velocity, not RMS."
                .into(),
        );
    }

    let parsed = parse_velocity_functions_file(Path::new(&input_path))?;
    if parsed.profiles.is_empty() {
        return Err("Velocity functions file did not contain any control profiles.".into());
    }

    let handle = open_store(&store_path)?;
    let coordinate_reference = handle
        .manifest
        .volume
        .coordinate_reference_binding
        .as_ref()
        .and_then(|binding| binding.effective.clone());
    let grid_transform = handle
        .manifest
        .volume
        .spatial
        .as_ref()
        .and_then(|spatial| spatial.grid_transform.clone());
    let source_stem = file_stem_from_path(&input_path);
    let output_slug = slugify(&format!(
        "{}-{}",
        source_stem,
        velocity_quantity_kind_slug(velocity_kind)
    ));
    let control_profile_set_id = format!("{output_slug}-control-profiles");
    let model = LayeredVelocityModel {
        id: format!("{output_slug}-layered-model"),
        name: format!(
            "{} {} Control Profiles",
            display_name_from_stem(&source_stem),
            velocity_quantity_kind_label(velocity_kind)
        ),
        derived_from: vec![handle.dataset_id().0.clone(), input_path.clone()],
        coordinate_reference: coordinate_reference.clone(),
        grid_transform: grid_transform.clone(),
        vertical_domain: TimeDepthDomain::Time,
        travel_time_reference: Some(TravelTimeReference::TwoWay),
        depth_reference: Some(DepthReferenceKind::TrueVerticalDepth),
        intervals: vec![LayeredVelocityInterval {
            id: format!("{output_slug}-survey-interval"),
            name: "Survey interval".to_string(),
            top_boundary: StratigraphicBoundaryReference::SurveyTop,
            base_boundary: StratigraphicBoundaryReference::SurveyBase,
            trend: VelocityIntervalTrend::Constant {
                velocity_m_per_s: 1500.0,
            },
            control_profile_set_id: Some(control_profile_set_id.clone()),
            control_profile_velocity_kind: Some(velocity_kind),
            lateral_interpolation: Some(LateralInterpolationMethod::Nearest),
            vertical_interpolation: Some(VerticalInterpolationMethod::Linear),
            control_blend_weight: Some(1.0),
            notes: vec![
                "Built from sparse velocity control profiles imported from text.".to_string(),
            ],
        }],
        notes: vec![
            "Single-interval authored model compiled from sparse control profiles.".to_string(),
            "Current builder path uses nearest lateral interpolation and linear vertical interpolation."
                .to_string(),
        ],
    };
    let request = BuildSurveyTimeDepthTransformRequest {
        schema_version: IPC_SCHEMA_VERSION,
        store_path: store_path.clone(),
        model,
        control_profile_sets: vec![VelocityControlProfileSet {
            id: control_profile_set_id,
            name: format!(
                "{} {} Profiles",
                display_name_from_stem(&source_stem),
                velocity_quantity_kind_label(velocity_kind)
            ),
            derived_from: vec![input_path.clone()],
            coordinate_reference,
            travel_time_reference: TravelTimeReference::TwoWay,
            depth_reference: DepthReferenceKind::TrueVerticalDepth,
            profiles: parsed.profiles.clone(),
            notes: vec![
                "Imported from Velocity_functions.txt style sparse profile file.".to_string(),
            ],
        }],
        output_id: Some(format!("{output_slug}-survey-transform")),
        output_name: Some(format!(
            "{} {} Transform",
            display_name_from_stem(&source_stem),
            velocity_quantity_kind_label(velocity_kind)
        )),
        preferred_velocity_kind: Some(velocity_kind),
        output_depth_unit: "m".to_string(),
        notes: vec![
            format!("Imported from {}", Path::new(&input_path).display()),
            "Compiled from sparse control profiles through the authored-model builder.".to_string(),
        ],
    };
    let model = build_survey_time_depth_transform(&request)?;

    Ok(ImportVelocityFunctionsModelResponse {
        schema_version: IPC_SCHEMA_VERSION,
        input_path,
        velocity_kind,
        profile_count: parsed.profiles.len(),
        sample_count: parsed.sample_count,
        model,
    })
}

fn normalized_index(index: usize, count: usize) -> f32 {
    if count <= 1 {
        0.0
    } else {
        index as f32 / (count - 1) as f32
    }
}

fn inferred_sample_interval_ms(sample_axis_ms: &[f32]) -> f32 {
    if sample_axis_ms.len() >= 2 {
        sample_axis_ms[1] - sample_axis_ms[0]
    } else {
        0.0
    }
}

fn distance_squared(x: f32, y: f32, center_x: f32, center_y: f32) -> f32 {
    let dx = x - center_x;
    let dy = y - center_y;
    dx * dx + dy * dy
}

fn parse_velocity_functions_file(
    input_path: &Path,
) -> Result<ParsedVelocityFunctions, Box<dyn std::error::Error>> {
    let contents = fs::read_to_string(input_path)?;
    let mut rows_by_profile = HashMap::<(u64, u64), Vec<ParsedVelocityProfileRow>>::new();
    let mut sample_count = 0_usize;

    for (line_index, raw_line) in contents.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty()
            || line.starts_with('#')
            || line.starts_with("This data contains")
            || line.starts_with("CDP-X")
        {
            continue;
        }

        let columns = line
            .split(|character: char| character.is_whitespace() || character == ',')
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        if columns.len() < 7 {
            return Err(format!(
                "Velocity functions row {} is invalid: expected at least 7 columns, found {}.",
                line_index + 1,
                columns.len()
            )
            .into());
        }

        let x = columns[0].parse::<f64>().map_err(|error| {
            format!(
                "Velocity functions row {} has invalid X coordinate '{}': {error}",
                line_index + 1,
                columns[0]
            )
        })?;
        let y = columns[1].parse::<f64>().map_err(|error| {
            format!(
                "Velocity functions row {} has invalid Y coordinate '{}': {error}",
                line_index + 1,
                columns[1]
            )
        })?;
        let time_ms = columns[2].parse::<f32>().map_err(|error| {
            format!(
                "Velocity functions row {} has invalid time '{}': {error}",
                line_index + 1,
                columns[2]
            )
        })?;
        let vrms_m_per_s = columns[3].parse::<f32>().map_err(|error| {
            format!(
                "Velocity functions row {} has invalid Vrms '{}': {error}",
                line_index + 1,
                columns[3]
            )
        })?;
        let vint_m_per_s = columns[4].parse::<f32>().map_err(|error| {
            format!(
                "Velocity functions row {} has invalid Vint '{}': {error}",
                line_index + 1,
                columns[4]
            )
        })?;
        let vavg_m_per_s = columns[5].parse::<f32>().map_err(|error| {
            format!(
                "Velocity functions row {} has invalid Vavg '{}': {error}",
                line_index + 1,
                columns[5]
            )
        })?;
        let depth_m = columns[6].parse::<f32>().map_err(|error| {
            format!(
                "Velocity functions row {} has invalid depth '{}': {error}",
                line_index + 1,
                columns[6]
            )
        })?;

        rows_by_profile
            .entry((x.to_bits(), y.to_bits()))
            .or_default()
            .push(ParsedVelocityProfileRow {
                x,
                y,
                sample: VelocityControlProfileSample {
                    time_ms,
                    depth_m: Some(depth_m),
                    vrms_m_per_s: Some(vrms_m_per_s),
                    vint_m_per_s: Some(vint_m_per_s),
                    vavg_m_per_s: Some(vavg_m_per_s),
                },
            });
        sample_count += 1;
    }

    let mut profiles = rows_by_profile
        .into_values()
        .enumerate()
        .map(|(profile_index, mut rows)| {
            rows.sort_by(|left, right| left.sample.time_ms.total_cmp(&right.sample.time_ms));
            let first = rows
                .first()
                .ok_or_else(|| "Velocity profile group was unexpectedly empty.".to_string())?;
            Ok::<VelocityControlProfile, Box<dyn std::error::Error>>(VelocityControlProfile {
                id: format!("control-profile-{:05}", profile_index + 1),
                location: ProjectedPoint2 {
                    x: first.x,
                    y: first.y,
                },
                wellbore_id: None,
                samples: rows.into_iter().map(|row| row.sample).collect(),
                notes: Vec::new(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    profiles.sort_by(|left, right| {
        left.location
            .x
            .total_cmp(&right.location.x)
            .then(left.location.y.total_cmp(&right.location.y))
    });

    Ok(ParsedVelocityFunctions {
        profiles,
        sample_count,
    })
}

fn velocity_quantity_kind_label(kind: VelocityQuantityKind) -> &'static str {
    match kind {
        VelocityQuantityKind::Interval => "Interval",
        VelocityQuantityKind::Rms => "RMS",
        VelocityQuantityKind::Average => "Average",
    }
}

fn velocity_quantity_kind_slug(kind: VelocityQuantityKind) -> &'static str {
    match kind {
        VelocityQuantityKind::Interval => "vint",
        VelocityQuantityKind::Rms => "vrms",
        VelocityQuantityKind::Average => "vavg",
    }
}

fn file_stem_from_path(file_path: &str) -> String {
    Path::new(file_path)
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| "velocity-functions".to_string())
}

fn display_name_from_stem(stem: &str) -> String {
    stem.replace('_', " ").trim().to_string()
}

fn slugify(value: &str) -> String {
    let normalized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    let slug = normalized
        .split('-')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() {
        "velocity-functions".to_string()
    } else {
        slug
    }
}

pub fn load_gather(
    store_path: String,
    request: GatherRequest,
) -> Result<GatherView, Box<dyn std::error::Error>> {
    let handle = open_prestack_store(&store_path)?;
    ensure_prestack_dataset_matches(&handle, &request.dataset_id.0)?;
    Ok(prestack_gather_view(&store_path, &request)?)
}

pub fn preview_processing(
    request: PreviewTraceLocalProcessingRequest,
) -> Result<PreviewTraceLocalProcessingResponse, Box<dyn std::error::Error>> {
    let handle = open_store(&request.store_path)?;
    ensure_dataset_matches(&handle, &request.section.dataset_id.0)?;
    let section = preview_processing_section_view(
        &request.store_path,
        request.section.axis,
        request.section.index,
        &request.pipeline,
    )?;
    Ok(PreviewTraceLocalProcessingResponse {
        schema_version: IPC_SCHEMA_VERSION,
        preview: PreviewView {
            section,
            processing_label: preview_processing_label(&request.pipeline),
            preview_ready: true,
        },
        pipeline: request.pipeline,
    })
}

pub fn preview_subvolume_processing(
    request: PreviewSubvolumeProcessingRequest,
) -> Result<PreviewSubvolumeProcessingResponse, Box<dyn std::error::Error>> {
    let handle = open_store(&request.store_path)?;
    ensure_dataset_matches(&handle, &request.section.dataset_id.0)?;
    let section = preview_subvolume_processing_section_view(
        &request.store_path,
        request.section.axis,
        request.section.index,
        &request.pipeline,
    )?;
    Ok(PreviewSubvolumeProcessingResponse {
        schema_version: IPC_SCHEMA_VERSION,
        preview: PreviewView {
            section,
            processing_label: preview_subvolume_processing_label(&request.pipeline),
            preview_ready: true,
        },
        pipeline: request.pipeline,
    })
}

pub fn preview_gather_processing(
    request: PreviewGatherProcessingRequest,
) -> Result<PreviewGatherProcessingResponse, Box<dyn std::error::Error>> {
    let handle = open_prestack_store(&request.store_path)?;
    ensure_prestack_dataset_matches(&handle, &request.gather.dataset_id.0)?;
    let preview =
        preview_gather_processing_view(&request.store_path, &request.gather, &request.pipeline)?;
    Ok(PreviewGatherProcessingResponse {
        schema_version: IPC_SCHEMA_VERSION,
        preview,
        pipeline: request.pipeline,
    })
}

pub fn apply_processing(
    request: RunTraceLocalProcessingRequest,
) -> Result<DatasetSummary, Box<dyn std::error::Error>> {
    let pipeline = request.pipeline;
    let output_store = request
        .output_store_path
        .map(PathBuf::from)
        .unwrap_or_else(|| default_output_store_path(&request.store_path, &pipeline));
    prepare_processing_output_store(&output_store, request.overwrite_existing)?;
    let materialize_options = materialize_options_for_store(&request.store_path)?;
    let derived = materialize_processing_volume(
        &request.store_path,
        &output_store,
        &pipeline,
        materialize_options,
    )?;
    Ok(DatasetSummary {
        store_path: derived.root.to_string_lossy().into_owned(),
        descriptor: handle_for_summary(&derived)?,
    })
}

pub fn apply_subvolume_processing(
    request: RunSubvolumeProcessingRequest,
) -> Result<DatasetSummary, Box<dyn std::error::Error>> {
    let pipeline = request.pipeline;
    let output_store = request
        .output_store_path
        .map(PathBuf::from)
        .unwrap_or_else(|| default_subvolume_output_store_path(&request.store_path, &pipeline));
    prepare_processing_output_store(&output_store, request.overwrite_existing)?;
    let materialize_options = materialize_options_for_store(&request.store_path)?;
    let derived = materialize_subvolume_processing_volume(
        &request.store_path,
        &output_store,
        &pipeline,
        materialize_options,
    )?;
    Ok(DatasetSummary {
        store_path: derived.root.to_string_lossy().into_owned(),
        descriptor: handle_for_summary(&derived)?,
    })
}

pub fn apply_gather_processing(
    request: RunGatherProcessingRequest,
) -> Result<DatasetSummary, Box<dyn std::error::Error>> {
    let pipeline = request.pipeline;
    let output_store = request
        .output_store_path
        .map(PathBuf::from)
        .unwrap_or_else(|| default_gather_output_store_path(&request.store_path, &pipeline));
    prepare_processing_output_store(&output_store, request.overwrite_existing)?;
    let derived =
        materialize_gather_processing_store(&request.store_path, &output_store, &pipeline)?;
    dataset_summary_for_path(&derived.root)
}

pub fn amplitude_spectrum(
    request: AmplitudeSpectrumRequest,
) -> Result<AmplitudeSpectrumResponse, Box<dyn std::error::Error>> {
    let handle = open_store(&request.store_path)?;
    ensure_dataset_matches(&handle, &request.section.dataset_id.0)?;
    let curve = amplitude_spectrum_from_store(
        &request.store_path,
        request.section.axis,
        request.section.index,
        request
            .pipeline
            .as_ref()
            .map(|pipeline| pipeline.operations().cloned().collect::<Vec<_>>())
            .as_deref(),
        &request.selection,
    )?;

    Ok(AmplitudeSpectrumResponse {
        schema_version: IPC_SCHEMA_VERSION,
        section: request.section,
        selection: request.selection,
        sample_interval_ms: handle.volume_descriptor().sample_interval_ms,
        curve,
        processing_label: request.pipeline.as_ref().map(preview_processing_label),
    })
}

pub fn run_velocity_scan(
    request: VelocityScanRequest,
) -> Result<VelocityScanResponse, Box<dyn std::error::Error>> {
    let handle = open_prestack_store(&request.store_path)?;
    ensure_prestack_dataset_matches(&handle, &request.gather.dataset_id.0)?;
    Ok(velocity_scan(request)?)
}

pub fn default_output_store_path(
    input_store_path: impl AsRef<Path>,
    pipeline: &TraceLocalProcessingPipeline,
) -> PathBuf {
    let input_store_path = input_store_path.as_ref();
    let parent = input_store_path.parent().unwrap_or_else(|| Path::new("."));
    let stem = input_store_path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("dataset");
    let suffix = pipeline_slug(pipeline);
    parent.join(format!("{stem}.{suffix}.tbvol"))
}

pub fn default_export_segy_path(input_store_path: impl AsRef<Path>) -> PathBuf {
    let input_store_path = input_store_path.as_ref();
    let parent = input_store_path.parent().unwrap_or_else(|| Path::new("."));
    let stem = input_store_path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("dataset");
    parent.join(format!("{stem}.export.sgy"))
}

pub fn default_export_zarr_path(input_store_path: impl AsRef<Path>) -> PathBuf {
    let input_store_path = input_store_path.as_ref();
    let parent = input_store_path.parent().unwrap_or_else(|| Path::new("."));
    let stem = input_store_path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("dataset");
    parent.join(format!("{stem}.export.zarr"))
}

pub fn default_subvolume_output_store_path(
    input_store_path: impl AsRef<Path>,
    pipeline: &SubvolumeProcessingPipeline,
) -> PathBuf {
    let input_store_path = input_store_path.as_ref();
    let parent = input_store_path.parent().unwrap_or_else(|| Path::new("."));
    let stem = input_store_path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("dataset");
    let suffix = subvolume_pipeline_slug(pipeline);
    parent.join(format!("{stem}.{suffix}.tbvol"))
}

pub fn default_gather_output_store_path(
    input_store_path: impl AsRef<Path>,
    pipeline: &GatherProcessingPipeline,
) -> PathBuf {
    let input_store_path = input_store_path.as_ref();
    let parent = input_store_path.parent().unwrap_or_else(|| Path::new("."));
    let stem = input_store_path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("dataset");
    let suffix = gather_pipeline_slug(pipeline);
    parent.join(format!("{stem}.{suffix}.tbgath"))
}

fn dataset_summary_for_path(
    store_path: impl AsRef<Path>,
) -> Result<DatasetSummary, Box<dyn std::error::Error>> {
    let store_path = store_path.as_ref();
    let descriptor = match open_store(store_path) {
        Ok(_) => describe_store(store_path)?,
        Err(poststack_error) => match open_prestack_store(store_path) {
            Ok(_) => describe_prestack_store(store_path)?,
            Err(prestack_error) => {
                return Err(format!(
                    "failed to open dataset store as tbvol ({poststack_error}) or tbgath ({prestack_error})"
                )
                .into())
            }
        },
    };
    Ok(DatasetSummary {
        store_path: store_path.to_string_lossy().into_owned(),
        descriptor,
    })
}

fn prepare_export_output_path(
    input_store_path: &Path,
    output_path: &Path,
    overwrite_existing: bool,
    output_label: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let input_store_path = input_store_path
        .canonicalize()
        .unwrap_or_else(|_| input_store_path.to_path_buf());
    let output_path = output_path
        .canonicalize()
        .unwrap_or_else(|_| output_path.to_path_buf());

    if input_store_path == output_path {
        return Err(
            format!("Output {output_label} path cannot overwrite the input tbvol store.").into(),
        );
    }

    if !overwrite_existing || !output_path.exists() {
        return Ok(());
    }

    let metadata = fs::symlink_metadata(&output_path)?;
    if metadata.file_type().is_dir() {
        fs::remove_dir_all(&output_path)?;
        return Ok(());
    }

    fs::remove_file(&output_path)?;
    Ok(())
}

fn suggested_action(action: seis_runtime::PreflightAction) -> SuggestedImportAction {
    match action {
        seis_runtime::PreflightAction::DirectDenseIngest => {
            SuggestedImportAction::DirectDenseIngest
        }
        seis_runtime::PreflightAction::RegularizeSparseSurvey => {
            SuggestedImportAction::RegularizeSparseSurvey
        }
        seis_runtime::PreflightAction::ReviewGeometryMapping => {
            SuggestedImportAction::ReviewGeometryMapping
        }
        seis_runtime::PreflightAction::UnsupportedInV1 => SuggestedImportAction::UnsupportedInV1,
    }
}

fn preflight_response(
    input_path: String,
    preflight: &seis_runtime::SurveyPreflight,
    suggested_geometry_override: Option<SegyGeometryOverride>,
    geometry_candidates: Vec<SegyGeometryCandidate>,
) -> SurveyPreflightResponse {
    let mut notes = preflight.notes.clone();
    if !geometry_candidates.is_empty() {
        notes.push("TraceBoost found one or more alternate header mappings that may allow import without manual SEG-Y repair.".to_string());
    }
    if suggested_geometry_override.is_some() {
        notes.push(
            "A single high-confidence alternate mapping was detected; review it before import."
                .to_string(),
        );
    }

    SurveyPreflightResponse {
        schema_version: IPC_SCHEMA_VERSION,
        input_path,
        trace_count: preflight.inspection.trace_count,
        samples_per_trace: preflight.inspection.samples_per_trace as usize,
        sample_data_fidelity: preflight.sample_data_fidelity.clone(),
        classification: preflight.geometry.classification.clone(),
        stacking_state: preflight.geometry.stacking_state.clone(),
        organization: preflight.geometry.organization.clone(),
        layout: preflight.geometry.layout.clone(),
        gather_axis_kind: preflight.geometry.gather_axis_kind.clone(),
        suggested_action: suggested_action(preflight.recommended_action),
        observed_trace_count: preflight.geometry.observed_trace_count,
        expected_trace_count: preflight.geometry.expected_trace_count,
        completeness_ratio: preflight.geometry.completeness_ratio,
        resolved_geometry: geometry_override_from_preflight(preflight),
        suggested_geometry_override,
        geometry_candidates,
        notes,
    }
}

fn discover_geometry_candidates(
    input_path: &str,
    baseline: &seis_runtime::SurveyPreflight,
) -> Vec<SegyGeometryCandidate> {
    let baseline_geometry = geometry_override_from_preflight(baseline);
    let mut seen = HashSet::new();
    let mut candidates = Vec::new();

    for spec in GEOMETRY_CANDIDATE_SPECS {
        let geometry = SegyGeometryOverride {
            inline_3d: Some(SegyHeaderField {
                start_byte: spec.inline.0,
                value_type: spec.inline.1.clone(),
            }),
            crossline_3d: Some(SegyHeaderField {
                start_byte: spec.crossline.0,
                value_type: spec.crossline.1.clone(),
            }),
            third_axis: None,
        };
        if geometry == baseline_geometry {
            continue;
        }

        let preflight = match preflight_segy(
            input_path,
            &ingest_options_from_geometry_override(Some(&geometry)),
        ) {
            Ok(preflight) => preflight,
            Err(_) => continue,
        };

        let action = suggested_action(preflight.recommended_action);
        if !matches!(
            action,
            SuggestedImportAction::DirectDenseIngest
                | SuggestedImportAction::RegularizeSparseSurvey
        ) {
            continue;
        }
        if !is_plausible_geometry_candidate(&preflight) {
            continue;
        }

        let geometry_key = (
            preflight.geometry.inline_field.start_byte,
            preflight.geometry.inline_field.value_type.clone(),
            preflight.geometry.crossline_field.start_byte,
            preflight.geometry.crossline_field.value_type.clone(),
            preflight
                .geometry
                .third_axis_field
                .as_ref()
                .map(|field| (field.start_byte, field.value_type.clone())),
        );
        if !seen.insert(geometry_key) {
            continue;
        }

        candidates.push(SegyGeometryCandidate {
            label: spec.label.to_string(),
            geometry: geometry_override_from_preflight(&preflight),
            classification: preflight.geometry.classification.clone(),
            stacking_state: preflight.geometry.stacking_state.clone(),
            organization: preflight.geometry.organization.clone(),
            layout: preflight.geometry.layout.clone(),
            gather_axis_kind: preflight.geometry.gather_axis_kind.clone(),
            suggested_action: action,
            inline_count: preflight.geometry.inline_count,
            crossline_count: preflight.geometry.crossline_count,
            third_axis_count: preflight.geometry.third_axis_count,
            observed_trace_count: preflight.geometry.observed_trace_count,
            expected_trace_count: preflight.geometry.expected_trace_count,
            completeness_ratio: preflight.geometry.completeness_ratio,
            auto_selectable: is_high_confidence_dense_candidate(&preflight),
            notes: preflight.notes.clone(),
        });
    }

    candidates.sort_by_key(|candidate| {
        (
            Reverse(geometry_candidate_rank(candidate)),
            Reverse(
                candidate
                    .inline_count
                    .saturating_mul(candidate.crossline_count),
            ),
            candidate.label.clone(),
        )
    });
    candidates
}

fn geometry_candidate_rank(candidate: &SegyGeometryCandidate) -> usize {
    let action_score = match candidate.suggested_action {
        SuggestedImportAction::DirectDenseIngest => 3,
        SuggestedImportAction::RegularizeSparseSurvey => 2,
        SuggestedImportAction::ReviewGeometryMapping => 1,
        SuggestedImportAction::UnsupportedInV1 => 0,
    };
    let auto_score = usize::from(candidate.auto_selectable);
    let axis_balance_score = candidate
        .inline_count
        .min(candidate.crossline_count)
        .min(10_000);
    (action_score * 10_000)
        + (auto_score * 1_000)
        + axis_balance_score
        + ((candidate.completeness_ratio * 100.0).round() as usize)
}

fn preferred_geometry_override(
    candidates: &[SegyGeometryCandidate],
) -> Option<SegyGeometryOverride> {
    let mut auto_candidates = candidates
        .iter()
        .filter(|candidate| candidate.auto_selectable);
    let first = auto_candidates.next()?;
    if auto_candidates.next().is_some() {
        return None;
    }
    Some(first.geometry.clone())
}

fn is_high_confidence_dense_candidate(preflight: &seis_runtime::SurveyPreflight) -> bool {
    matches!(
        preflight.recommended_action,
        seis_runtime::PreflightAction::DirectDenseIngest
    ) && preflight.geometry.observed_trace_count == preflight.geometry.expected_trace_count
        && preflight.geometry.inline_count > 1
        && preflight.geometry.crossline_count > 1
}

fn is_plausible_geometry_candidate(preflight: &seis_runtime::SurveyPreflight) -> bool {
    preflight.geometry.inline_count > 1 && preflight.geometry.crossline_count > 1
}

fn ingest_options_from_geometry_override(
    geometry_override: Option<&SegyGeometryOverride>,
) -> IngestOptions {
    IngestOptions {
        geometry: geometry_override_to_seis_options(geometry_override),
        ..IngestOptions::default()
    }
}

fn geometry_override_to_seis_options(
    geometry_override: Option<&SegyGeometryOverride>,
) -> SeisGeometryOptions {
    let mut geometry = SeisGeometryOptions::default();
    if let Some(geometry_override) = geometry_override {
        geometry.header_mapping.inline_3d = geometry_override
            .inline_3d
            .as_ref()
            .map(|field| contract_header_field_to_runtime("INLINE_3D", field));
        geometry.header_mapping.crossline_3d = geometry_override
            .crossline_3d
            .as_ref()
            .map(|field| contract_header_field_to_runtime("CROSSLINE_3D", field));
        geometry.third_axis_field = geometry_override
            .third_axis
            .as_ref()
            .map(|field| contract_header_field_to_runtime("THIRD_AXIS", field));
    }
    geometry
}

fn contract_header_field_to_runtime(name: &'static str, field: &SegyHeaderField) -> HeaderField {
    match field.value_type {
        SegyHeaderValueType::I16 => HeaderField::new_i16(name, field.start_byte),
        SegyHeaderValueType::I32 => HeaderField::new_i32(name, field.start_byte),
    }
}

fn geometry_override_from_preflight(
    preflight: &seis_runtime::SurveyPreflight,
) -> SegyGeometryOverride {
    SegyGeometryOverride {
        inline_3d: Some(contract_header_field_from_spec(
            &preflight.geometry.inline_field,
        )),
        crossline_3d: Some(contract_header_field_from_spec(
            &preflight.geometry.crossline_field,
        )),
        third_axis: preflight
            .geometry
            .third_axis_field
            .as_ref()
            .map(contract_header_field_from_spec),
    }
}

fn contract_header_field_from_spec(spec: &seis_runtime::HeaderFieldSpec) -> SegyHeaderField {
    SegyHeaderField {
        start_byte: spec.start_byte,
        value_type: contract_header_value_type(&spec.value_type),
    }
}

fn contract_header_value_type(value_type: &str) -> SegyHeaderValueType {
    match value_type {
        "I16" => SegyHeaderValueType::I16,
        _ => SegyHeaderValueType::I32,
    }
}

fn handle_for_summary(
    handle: &seis_runtime::StoreHandle,
) -> Result<seis_runtime::VolumeDescriptor, Box<dyn std::error::Error>> {
    Ok(describe_store(&handle.root)?)
}

fn ensure_dataset_matches(
    handle: &seis_runtime::StoreHandle,
    expected_dataset_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let actual = handle.dataset_id().0;
    if actual != expected_dataset_id {
        return Err(format!(
            "Section request dataset mismatch: expected {expected_dataset_id}, found {actual}"
        )
        .into());
    }
    Ok(())
}

fn ensure_prestack_dataset_matches(
    handle: &seis_runtime::PrestackStoreHandle,
    expected_dataset_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let actual = handle.dataset_id().0;
    if actual != expected_dataset_id {
        return Err(format!(
            "Gather request dataset mismatch: expected {expected_dataset_id}, found {actual}"
        )
        .into());
    }
    Ok(())
}

pub fn preview_processing_label(pipeline: &TraceLocalProcessingPipeline) -> String {
    pipeline
        .name
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| pipeline_slug(pipeline))
}

pub fn preview_subvolume_processing_label(pipeline: &SubvolumeProcessingPipeline) -> String {
    pipeline
        .name
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| subvolume_pipeline_slug(pipeline))
}

fn pipeline_slug(pipeline: &TraceLocalProcessingPipeline) -> String {
    let mut parts = Vec::with_capacity(pipeline.operation_count());
    for operation in pipeline.operations() {
        let label = match operation {
            seis_runtime::ProcessingOperation::AmplitudeScalar { factor } => {
                format!("amplitude-scalar-{}", format_factor(*factor))
            }
            seis_runtime::ProcessingOperation::TraceRmsNormalize => {
                "trace-rms-normalize".to_string()
            }
            seis_runtime::ProcessingOperation::AgcRms { window_ms } => {
                format!("agc-rms-{}", format_factor(*window_ms))
            }
            seis_runtime::ProcessingOperation::PhaseRotation { angle_degrees } => {
                format!("phase-rotation-{}", format_factor(*angle_degrees))
            }
            seis_runtime::ProcessingOperation::LowpassFilter { f3_hz, f4_hz, .. } => format!(
                "lowpass-{}-{}",
                format_factor(*f3_hz),
                format_factor(*f4_hz)
            ),
            seis_runtime::ProcessingOperation::HighpassFilter { f1_hz, f2_hz, .. } => format!(
                "highpass-{}-{}",
                format_factor(*f1_hz),
                format_factor(*f2_hz)
            ),
            seis_runtime::ProcessingOperation::BandpassFilter {
                f1_hz,
                f2_hz,
                f3_hz,
                f4_hz,
                ..
            } => format!(
                "bandpass-{}-{}-{}-{}",
                format_factor(*f1_hz),
                format_factor(*f2_hz),
                format_factor(*f3_hz),
                format_factor(*f4_hz)
            ),
            seis_runtime::ProcessingOperation::VolumeArithmetic {
                operator,
                secondary_store_path,
            } => format!(
                "volume-{}-{}",
                volume_arithmetic_operator_slug(*operator),
                store_path_slug(secondary_store_path)
            ),
        };
        parts.push(label);
    }
    if parts.is_empty() {
        "pipeline".to_string()
    } else {
        parts.join("__")
    }
}

fn subvolume_pipeline_slug(pipeline: &SubvolumeProcessingPipeline) -> String {
    if let Some(name) = pipeline
        .name
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        return name.replace(' ', "-").to_ascii_lowercase();
    }

    let mut parts = Vec::new();
    if let Some(trace_local_pipeline) = pipeline.trace_local_pipeline.as_ref() {
        parts.push(pipeline_slug(trace_local_pipeline));
    }
    parts.push(format!(
        "crop-il-{}-{}-xl-{}-{}-z-{}-{}",
        pipeline.crop.inline_min,
        pipeline.crop.inline_max,
        pipeline.crop.xline_min,
        pipeline.crop.xline_max,
        format_factor(pipeline.crop.z_min_ms),
        format_factor(pipeline.crop.z_max_ms)
    ));
    parts.join("__")
}

fn gather_pipeline_slug(pipeline: &GatherProcessingPipeline) -> String {
    if let Some(name) = pipeline
        .name
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        return name.replace(' ', "-").to_ascii_lowercase();
    }

    let mut parts = Vec::new();
    if let Some(trace_local_pipeline) = pipeline.trace_local_pipeline.as_ref() {
        parts.push(pipeline_slug(trace_local_pipeline));
    }
    for operation in &pipeline.operations {
        let label = match operation {
            seis_runtime::GatherProcessingOperation::NmoCorrection {
                velocity_model,
                interpolation,
            } => format!(
                "nmo-{}-{}",
                velocity_model_slug(velocity_model),
                interpolation_slug(*interpolation)
            ),
            seis_runtime::GatherProcessingOperation::StretchMute {
                velocity_model,
                max_stretch_ratio,
            } => format!(
                "stretch-mute-{}-{}",
                velocity_model_slug(velocity_model),
                format_factor(*max_stretch_ratio)
            ),
            seis_runtime::GatherProcessingOperation::OffsetMute {
                min_offset,
                max_offset,
            } => format!(
                "offset-mute-{}-{}",
                optional_factor_slug(*min_offset),
                optional_factor_slug(*max_offset)
            ),
        };
        parts.push(label);
    }
    if parts.is_empty() {
        "gather-processing".to_string()
    } else {
        parts.join("__")
    }
}

fn interpolation_slug(mode: GatherInterpolationMode) -> &'static str {
    match mode {
        GatherInterpolationMode::Linear => "linear",
    }
}

fn velocity_model_slug(model: &VelocityFunctionSource) -> String {
    match model {
        VelocityFunctionSource::ConstantVelocity { velocity_m_per_s } => {
            format!("constant-{}", format_factor(*velocity_m_per_s))
        }
        VelocityFunctionSource::TimeVelocityPairs { .. } => "time-velocity-pairs".to_string(),
        VelocityFunctionSource::VelocityAssetReference { asset_id } => {
            format!(
                "velocity-asset-{}",
                asset_id.replace(' ', "-").to_ascii_lowercase()
            )
        }
    }
}

fn optional_factor_slug(value: Option<f32>) -> String {
    value
        .map(format_factor)
        .unwrap_or_else(|| "none".to_string())
}

fn volume_arithmetic_operator_slug(
    operator: seis_runtime::TraceLocalVolumeArithmeticOperator,
) -> &'static str {
    match operator {
        seis_runtime::TraceLocalVolumeArithmeticOperator::Add => "add",
        seis_runtime::TraceLocalVolumeArithmeticOperator::Subtract => "subtract",
        seis_runtime::TraceLocalVolumeArithmeticOperator::Multiply => "multiply",
        seis_runtime::TraceLocalVolumeArithmeticOperator::Divide => "divide",
    }
}

fn store_path_slug(store_path: &str) -> String {
    Path::new(store_path)
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            value
                .chars()
                .map(|ch| {
                    if ch.is_ascii_alphanumeric() {
                        ch.to_ascii_lowercase()
                    } else {
                        '-'
                    }
                })
                .collect::<String>()
        })
        .map(|value| {
            value
                .split('-')
                .filter(|segment| !segment.is_empty())
                .collect::<Vec<_>>()
                .join("-")
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "volume".to_string())
}

fn format_factor(value: f32) -> String {
    let mut formatted = format!("{value:.4}");
    while formatted.contains('.') && formatted.ends_with('0') {
        formatted.pop();
    }
    if formatted.ends_with('.') {
        formatted.pop();
    }
    formatted.replace('.', "_")
}

fn prestack_third_axis_field(field: PrestackThirdAxisField) -> HeaderField {
    match field {
        PrestackThirdAxisField::Offset => HeaderField::OFFSET,
    }
}

fn prepare_processing_output_store(
    output_path: &Path,
    overwrite_existing: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if !output_path.exists() {
        return Ok(());
    }
    if !overwrite_existing {
        return Err(format!(
            "Output processing store already exists: {}",
            output_path.display()
        )
        .into());
    }
    let metadata = fs::symlink_metadata(output_path)?;
    if metadata.file_type().is_dir() {
        fs::remove_dir_all(output_path)?;
    } else {
        fs::remove_file(output_path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn legacy_tbvol_fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test-data/f3.tbvol")
    }

    fn zarr_fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test-data/survey.zarr")
    }

    #[test]
    fn import_dataset_imports_zarr_fixture_to_tbvol_when_available() {
        let fixture = zarr_fixture_path();
        if !fixture.exists() {
            return;
        }

        let temp = tempdir().expect("temp dir");
        let output = temp.path().join("survey.tbvol");
        let response = import_dataset(ImportDatasetRequest {
            schema_version: IPC_SCHEMA_VERSION,
            input_path: fixture.display().to_string(),
            output_store_path: output.display().to_string(),
            geometry_override: None,
            overwrite_existing: false,
        })
        .expect("zarr fixture should import");

        assert_eq!(response.dataset.descriptor.shape, [23, 18, 75]);
        assert_eq!(response.dataset.descriptor.chunk_shape[2], 75);
    }

    #[test]
    fn export_dataset_zarr_roundtrips_legacy_tbvol_fixture() {
        let fixture = legacy_tbvol_fixture_path();
        if !fixture.exists() {
            return;
        }

        let temp = tempdir().expect("temp dir");
        let exported = temp.path().join("f3-export.zarr");
        let reimported = temp.path().join("f3-export-import.tbvol");

        let export_response = export_dataset_zarr(
            fixture.display().to_string(),
            exported.display().to_string(),
            false,
        )
        .expect("legacy tbvol fixture should export to zarr");
        assert_eq!(PathBuf::from(&export_response.output_path), exported);

        let import_response = import_dataset(ImportDatasetRequest {
            schema_version: IPC_SCHEMA_VERSION,
            input_path: exported.display().to_string(),
            output_store_path: reimported.display().to_string(),
            geometry_override: None,
            overwrite_existing: false,
        })
        .expect("exported zarr should import");

        let reopened = open_dataset_summary(OpenDatasetRequest {
            schema_version: IPC_SCHEMA_VERSION,
            store_path: reimported.display().to_string(),
        })
        .expect("reimported tbvol should open");

        assert_eq!(import_response.dataset.descriptor.shape, [23, 18, 75]);
        assert_eq!(
            reopened.dataset.descriptor.geometry.fingerprint,
            import_response.dataset.descriptor.geometry.fingerprint
        );
        assert_eq!(
            reopened.dataset.descriptor.sample_interval_ms,
            import_response.dataset.descriptor.sample_interval_ms
        );
    }

    #[test]
    fn parse_velocity_functions_file_groups_sparse_profiles() {
        let temp = tempdir().expect("temp dir");
        let input = temp.path().join("Velocity_functions.txt");
        fs::write(
            &input,
            [
                "This data contains example velocities, not measured velocities",
                "CDP-X       CDP-Y   Time(ms)  Vrms    Vint    Vavg   Depth(m)",
                " 605882.71  6073657.74   50.00 1500.00 1500.00 1500.00   37.50",
                " 605882.71  6073657.74  858.86 1936.22 1960.00 1933.22  830.19",
                " 606082.63  6073663.33   50.00 1500.00 1500.00 1500.00   37.50",
                " 606082.63  6073663.33  859.57 1936.24 1960.00 1933.24  830.88",
            ]
            .join("\n"),
        )
        .expect("write sample velocity functions file");

        let parsed = parse_velocity_functions_file(&input).expect("parse velocity functions");
        assert_eq!(parsed.sample_count, 4);
        assert_eq!(parsed.profiles.len(), 2);
        assert_eq!(parsed.profiles[0].samples.len(), 2);
        assert_eq!(parsed.profiles[1].samples.len(), 2);
        assert_eq!(parsed.profiles[0].samples[0].vint_m_per_s, Some(1500.0));
        assert_eq!(parsed.profiles[0].samples[1].depth_m, Some(830.19));
    }
}

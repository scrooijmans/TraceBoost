use std::path::PathBuf;

use traceboost_app::{import_dataset, open_dataset_summary, preflight_dataset};

use clap::{Parser, Subcommand, ValueEnum};
use seis_contracts_interop::{
    IPC_SCHEMA_VERSION, ImportDatasetRequest, OpenDatasetRequest, SegyGeometryOverride,
    SegyHeaderField, SegyHeaderValueType, SurveyPreflightRequest,
};
use seis_runtime::{
    IngestOptions, SeisGeometryOptions, SparseSurveyPolicy, ValidationOptions, ingest_segy,
    inspect_segy, open_store, preflight_segy, run_validation,
};
use serde::Serialize;

#[derive(Debug, Parser)]
#[command(name = "traceboost-app")]
#[command(about = "Thin app-side shell for TraceBoost, backed by the in-repo runtime layer")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    BackendInfo,
    Inspect {
        input: PathBuf,
    },
    Analyze {
        input: PathBuf,
        #[arg(long)]
        inline_byte: Option<u16>,
        #[arg(long, value_enum, default_value_t = HeaderTypeArg::I32)]
        inline_type: HeaderTypeArg,
        #[arg(long)]
        crossline_byte: Option<u16>,
        #[arg(long, value_enum, default_value_t = HeaderTypeArg::I32)]
        crossline_type: HeaderTypeArg,
        #[arg(long)]
        third_axis_byte: Option<u16>,
        #[arg(long, value_enum, default_value_t = HeaderTypeArg::I32)]
        third_axis_type: HeaderTypeArg,
    },
    Ingest {
        input: PathBuf,
        output: PathBuf,
        #[arg(long, value_delimiter = ',')]
        chunk: Vec<usize>,
        #[arg(long)]
        inline_byte: Option<u16>,
        #[arg(long, value_enum, default_value_t = HeaderTypeArg::I32)]
        inline_type: HeaderTypeArg,
        #[arg(long)]
        crossline_byte: Option<u16>,
        #[arg(long, value_enum, default_value_t = HeaderTypeArg::I32)]
        crossline_type: HeaderTypeArg,
        #[arg(long)]
        third_axis_byte: Option<u16>,
        #[arg(long, value_enum, default_value_t = HeaderTypeArg::I32)]
        third_axis_type: HeaderTypeArg,
        #[arg(long)]
        regularize_sparse: bool,
        #[arg(long, default_value_t = 0.0)]
        fill_value: f32,
    },
    Validate {
        output: PathBuf,
        #[arg(long = "input")]
        inputs: Vec<PathBuf>,
    },
    PreflightImport {
        input: PathBuf,
        #[arg(long)]
        inline_byte: Option<u16>,
        #[arg(long, value_enum, default_value_t = HeaderTypeArg::I32)]
        inline_type: HeaderTypeArg,
        #[arg(long)]
        crossline_byte: Option<u16>,
        #[arg(long, value_enum, default_value_t = HeaderTypeArg::I32)]
        crossline_type: HeaderTypeArg,
        #[arg(long)]
        third_axis_byte: Option<u16>,
        #[arg(long, value_enum, default_value_t = HeaderTypeArg::I32)]
        third_axis_type: HeaderTypeArg,
    },
    ImportDataset {
        input: PathBuf,
        output: PathBuf,
        #[arg(long)]
        inline_byte: Option<u16>,
        #[arg(long, value_enum, default_value_t = HeaderTypeArg::I32)]
        inline_type: HeaderTypeArg,
        #[arg(long)]
        crossline_byte: Option<u16>,
        #[arg(long, value_enum, default_value_t = HeaderTypeArg::I32)]
        crossline_type: HeaderTypeArg,
        #[arg(long)]
        third_axis_byte: Option<u16>,
        #[arg(long, value_enum, default_value_t = HeaderTypeArg::I32)]
        third_axis_type: HeaderTypeArg,
        #[arg(long, default_value_t = false)]
        overwrite_existing: bool,
    },
    OpenDataset {
        store: PathBuf,
    },
    ViewSection {
        store: PathBuf,
        #[arg(value_enum)]
        axis: SectionAxisArg,
        index: usize,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum HeaderTypeArg {
    I16,
    I32,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum SectionAxisArg {
    Inline,
    Xline,
}

#[derive(Debug, Clone, Serialize)]
struct BackendInfo {
    backend_repo_hint: &'static str,
    backend_local_path_hint: &'static str,
    current_default_method_policy: &'static str,
    current_geometry_policy: &'static str,
    current_scope: &'static str,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Command::BackendInfo => {
            let info = BackendInfo {
                backend_repo_hint: "monorepo: runtime/",
                backend_local_path_hint: "../../runtime",
                current_default_method_policy: "keep linear as default unless a stronger method wins on every validation dataset",
                current_geometry_policy: "dense surveys ingest directly; sparse regular post-stack surveys require explicit regularization; duplicate-heavy surveys still stop for review",
                current_scope: "monorepo app shell with preflight and ingest routing; Tauri app not started yet",
            };
            println!("{}", serde_json::to_string_pretty(&info)?);
        }
        Command::Inspect { input } => {
            println!("{}", serde_json::to_string_pretty(&inspect_segy(input)?)?);
        }
        Command::Analyze {
            input,
            inline_byte,
            inline_type,
            crossline_byte,
            crossline_type,
            third_axis_byte,
            third_axis_type,
        } => {
            let options = IngestOptions {
                geometry: build_ingest_geometry(
                    inline_byte,
                    inline_type,
                    crossline_byte,
                    crossline_type,
                    third_axis_byte,
                    third_axis_type,
                ),
                ..IngestOptions::default()
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&preflight_segy(input, &options)?)?
            );
        }
        Command::Ingest {
            input,
            output,
            chunk,
            inline_byte,
            inline_type,
            crossline_byte,
            crossline_type,
            third_axis_byte,
            third_axis_type,
            regularize_sparse,
            fill_value,
        } => {
            let handle = ingest_segy(
                input,
                output,
                IngestOptions {
                    chunk_shape: parse_chunk_shape(&chunk),
                    geometry: build_ingest_geometry(
                        inline_byte,
                        inline_type,
                        crossline_byte,
                        crossline_type,
                        third_axis_byte,
                        third_axis_type,
                    ),
                    sparse_survey_policy: if regularize_sparse {
                        SparseSurveyPolicy::RegularizeToDense { fill_value }
                    } else {
                        SparseSurveyPolicy::default()
                    },
                    ..IngestOptions::default()
                },
            )?;
            println!("{}", serde_json::to_string_pretty(&handle.manifest)?);
        }
        Command::Validate { output, inputs } => {
            let summary = run_validation(ValidationOptions {
                output_dir: output,
                dataset_paths: inputs,
                validation_mode: seis_io::ValidationMode::Strict,
            })?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
        }
        Command::PreflightImport {
            input,
            inline_byte,
            inline_type,
            crossline_byte,
            crossline_type,
            third_axis_byte,
            third_axis_type,
        } => {
            let response = preflight_dataset(SurveyPreflightRequest {
                schema_version: IPC_SCHEMA_VERSION,
                input_path: input.to_string_lossy().into_owned(),
                geometry_override: build_geometry_override(
                    inline_byte,
                    inline_type,
                    crossline_byte,
                    crossline_type,
                    third_axis_byte,
                    third_axis_type,
                ),
            })?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        Command::ImportDataset {
            input,
            output,
            inline_byte,
            inline_type,
            crossline_byte,
            crossline_type,
            third_axis_byte,
            third_axis_type,
            overwrite_existing,
        } => {
            let response = import_dataset(ImportDatasetRequest {
                schema_version: IPC_SCHEMA_VERSION,
                input_path: input.to_string_lossy().into_owned(),
                output_store_path: output.to_string_lossy().into_owned(),
                geometry_override: build_geometry_override(
                    inline_byte,
                    inline_type,
                    crossline_byte,
                    crossline_type,
                    third_axis_byte,
                    third_axis_type,
                ),
                overwrite_existing,
            })?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        Command::OpenDataset { store } => {
            let response = open_dataset_summary(OpenDatasetRequest {
                schema_version: IPC_SCHEMA_VERSION,
                store_path: store.to_string_lossy().into_owned(),
            })?;
            println!("{}", serde_json::to_string_pretty(&response)?);
        }
        Command::ViewSection { store, axis, index } => {
            let view = open_store(store)?.section_view(axis.into(), index)?;
            println!("{}", serde_json::to_string(&view)?);
        }
    }

    Ok(())
}

impl From<SectionAxisArg> for seis_runtime::SectionAxis {
    fn from(value: SectionAxisArg) -> Self {
        match value {
            SectionAxisArg::Inline => Self::Inline,
            SectionAxisArg::Xline => Self::Xline,
        }
    }
}

fn parse_chunk_shape(values: &[usize]) -> [usize; 3] {
    match values {
        [a, b, c] => [*a, *b, *c],
        _ => [0, 0, 0],
    }
}

fn build_ingest_geometry(
    inline_byte: Option<u16>,
    inline_type: HeaderTypeArg,
    crossline_byte: Option<u16>,
    crossline_type: HeaderTypeArg,
    third_axis_byte: Option<u16>,
    third_axis_type: HeaderTypeArg,
) -> SeisGeometryOptions {
    let mut geometry = SeisGeometryOptions::default();
    geometry.header_mapping.inline_3d =
        inline_byte.map(|start_byte| header_field("INLINE_3D", start_byte, inline_type));
    geometry.header_mapping.crossline_3d =
        crossline_byte.map(|start_byte| header_field("CROSSLINE_3D", start_byte, crossline_type));
    geometry.third_axis_field =
        third_axis_byte.map(|start_byte| header_field("THIRD_AXIS", start_byte, third_axis_type));
    geometry
}

fn build_geometry_override(
    inline_byte: Option<u16>,
    inline_type: HeaderTypeArg,
    crossline_byte: Option<u16>,
    crossline_type: HeaderTypeArg,
    third_axis_byte: Option<u16>,
    third_axis_type: HeaderTypeArg,
) -> Option<SegyGeometryOverride> {
    let geometry = SegyGeometryOverride {
        inline_3d: inline_byte.map(|start_byte| SegyHeaderField {
            start_byte,
            value_type: segy_header_value_type(inline_type),
        }),
        crossline_3d: crossline_byte.map(|start_byte| SegyHeaderField {
            start_byte,
            value_type: segy_header_value_type(crossline_type),
        }),
        third_axis: third_axis_byte.map(|start_byte| SegyHeaderField {
            start_byte,
            value_type: segy_header_value_type(third_axis_type),
        }),
    };
    if geometry.inline_3d.is_none()
        && geometry.crossline_3d.is_none()
        && geometry.third_axis.is_none()
    {
        None
    } else {
        Some(geometry)
    }
}

fn segy_header_value_type(value_type: HeaderTypeArg) -> SegyHeaderValueType {
    match value_type {
        HeaderTypeArg::I16 => SegyHeaderValueType::I16,
        HeaderTypeArg::I32 => SegyHeaderValueType::I32,
    }
}

fn header_field(
    name: &'static str,
    start_byte: u16,
    value_type: HeaderTypeArg,
) -> seis_io::HeaderField {
    match value_type {
        HeaderTypeArg::I16 => seis_io::HeaderField::new_i16(name, start_byte),
        HeaderTypeArg::I32 => seis_io::HeaderField::new_i32(name, start_byte),
    }
}

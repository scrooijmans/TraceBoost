use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use seisrefine::{
    IngestOptions, SeisGeometryOptions, SparseSurveyPolicy, ValidationOptions, ingest_segy,
    inspect_segy, preflight_segy, run_validation,
};
use serde::Serialize;

#[derive(Debug, Parser)]
#[command(name = "traceboost-app")]
#[command(about = "Thin app-side shell for TraceBoost, backed by the standalone seisrefine repo")]
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
        #[arg(long, value_delimiter = ',', default_values_t = [16_usize, 16, 64])]
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
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum HeaderTypeArg {
    I16,
    I32,
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
                backend_repo_hint: "https://github.com/tuna-soup/seisrefine.git",
                backend_local_path_hint: "../seisrefine",
                current_default_method_policy:
                    "keep linear as default unless a stronger method wins on every validation dataset",
                current_geometry_policy:
                    "dense surveys ingest directly; sparse regular post-stack surveys require explicit regularization; duplicate-heavy surveys still stop for review",
                current_scope: "backend-first shell with preflight and ingest routing; Tauri app not started yet",
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
            println!("{}", serde_json::to_string_pretty(&preflight_segy(input, &options)?)?);
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
                        SparseSurveyPolicy::Reject
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
                validation_mode: sgyx::ValidationMode::Strict,
            })?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
        }
    }

    Ok(())
}

fn parse_chunk_shape(values: &[usize]) -> [usize; 3] {
    match values {
        [a, b, c] => [*a, *b, *c],
        _ => [16, 16, 64],
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

fn header_field(
    name: &'static str,
    start_byte: u16,
    value_type: HeaderTypeArg,
) -> sgyx::HeaderField {
    match value_type {
        HeaderTypeArg::I16 => sgyx::HeaderField::new_i16(name, start_byte),
        HeaderTypeArg::I32 => sgyx::HeaderField::new_i32(name, start_byte),
    }
}

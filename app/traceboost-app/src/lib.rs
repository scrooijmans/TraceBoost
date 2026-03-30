use std::path::{Path, PathBuf};

use seis_contracts_interop::{
    DatasetSummary, ImportDatasetRequest, ImportDatasetResponse, OpenDatasetRequest,
    OpenDatasetResponse, SuggestedImportAction, SurveyPreflightRequest, SurveyPreflightResponse,
    IPC_SCHEMA_VERSION,
};
use seis_runtime::{IngestOptions, SparseSurveyPolicy, describe_store, ingest_segy, open_store, preflight_segy};

pub fn preflight_dataset(
    request: SurveyPreflightRequest,
) -> Result<SurveyPreflightResponse, Box<dyn std::error::Error>> {
    let preflight = preflight_segy(&request.input_path, &IngestOptions::default())?;
    Ok(SurveyPreflightResponse {
        schema_version: IPC_SCHEMA_VERSION,
        input_path: request.input_path,
        trace_count: preflight.inspection.trace_count,
        samples_per_trace: preflight.inspection.samples_per_trace as usize,
        classification: preflight.geometry.classification,
        suggested_action: suggested_action(preflight.recommended_action),
        observed_trace_count: preflight.geometry.observed_trace_count,
        expected_trace_count: preflight.geometry.expected_trace_count,
        completeness_ratio: preflight.geometry.completeness_ratio,
        notes: preflight.notes,
    })
}

pub fn import_dataset(
    request: ImportDatasetRequest,
) -> Result<ImportDatasetResponse, Box<dyn std::error::Error>> {
    let output = PathBuf::from(&request.output_store_path);
    let handle = ingest_segy(
        &request.input_path,
        &output,
        IngestOptions {
            sparse_survey_policy: SparseSurveyPolicy::Reject,
            ..IngestOptions::default()
        },
    )?;
    Ok(ImportDatasetResponse {
        schema_version: IPC_SCHEMA_VERSION,
        dataset: DatasetSummary {
            store_path: handle.root.to_string_lossy().into_owned(),
            descriptor: describe_store(&output)?,
        },
    })
}

pub fn open_dataset_summary(
    request: OpenDatasetRequest,
) -> Result<OpenDatasetResponse, Box<dyn std::error::Error>> {
    let store_path = request.store_path;
    let descriptor = {
        let store = Path::new(&store_path);
        let _handle = open_store(store)?;
        describe_store(store)?
    };
    Ok(OpenDatasetResponse {
        schema_version: IPC_SCHEMA_VERSION,
        dataset: DatasetSummary {
            store_path,
            descriptor,
        },
    })
}

fn suggested_action(action: seis_runtime::PreflightAction) -> SuggestedImportAction {
    match action {
        seis_runtime::PreflightAction::DirectDenseIngest => SuggestedImportAction::DirectDenseIngest,
        seis_runtime::PreflightAction::RegularizeSparseSurvey => {
            SuggestedImportAction::RegularizeSparseSurvey
        }
        seis_runtime::PreflightAction::ReviewGeometryMapping => {
            SuggestedImportAction::ReviewGeometryMapping
        }
        seis_runtime::PreflightAction::UnsupportedInV1 => SuggestedImportAction::UnsupportedInV1,
    }
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DatasetId(pub String);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VolumeDescriptor {
    pub id: DatasetId,
    pub label: String,
    pub shape: [usize; 3],
    pub chunk_shape: [usize; 3],
    pub sample_interval_ms: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SectionAxis {
    Inline,
    Xline,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SectionRequest {
    pub dataset_id: DatasetId,
    pub axis: SectionAxis,
    pub index: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SectionTileRequest {
    pub section: SectionRequest,
    pub trace_range: [usize; 2],
    pub sample_range: [usize; 2],
    pub lod: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProcessingParameters {
    pub algorithm: String,
    pub gain: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InterpretationPoint {
    pub trace_index: usize,
    pub sample_index: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dataset_id_is_hashable_and_cloneable() {
        let id = DatasetId("demo".to_string());
        let cloned = id.clone();
        assert_eq!(id, cloned);
    }
}


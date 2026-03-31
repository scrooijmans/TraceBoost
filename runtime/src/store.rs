use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ndarray::{Array2, Array3, Axis, Ix2, Ix3};
use seis_contracts_core::{DatasetId, SectionAxis, VolumeDescriptor};
use seis_contracts_views::{
    SectionColorMap, SectionCoordinate, SectionDisplayDefaults, SectionMetadata, SectionPolarity,
    SectionRenderMode, SectionUnits, SectionView,
};
use serde_json::json;
use zarrs::array::{Array, ArrayBuilder, data_type};
use zarrs::filesystem::FilesystemStore;
use zarrs::group::GroupBuilder;
use zarrs::storage::ReadableWritableListableStorage;

use crate::error::SeisRefineError;
use crate::metadata::StoreManifest;

pub const ARRAY_PATH: &str = "/amplitude";
pub const OCCUPANCY_PATH: &str = "/occupancy";

#[derive(Debug, Clone)]
pub struct StoreHandle {
    pub root: PathBuf,
    pub manifest: StoreManifest,
}

impl StoreHandle {
    pub fn manifest_path(&self) -> PathBuf {
        self.root.join(StoreManifest::FILE_NAME)
    }

    pub fn dataset_id(&self) -> DatasetId {
        DatasetId(dataset_id_string(&self.root))
    }

    pub fn volume_descriptor(&self) -> VolumeDescriptor {
        VolumeDescriptor {
            id: self.dataset_id(),
            label: dataset_label(&self.root),
            shape: self.manifest.shape,
            chunk_shape: self.manifest.chunk_shape,
            sample_interval_ms: self.manifest.source.sample_interval_us as f32 / 1000.0,
        }
    }

    pub fn section_view(
        &self,
        axis: SectionAxis,
        index: usize,
    ) -> Result<SectionView, SeisRefineError> {
        let (axis_len, traces) = match axis {
            SectionAxis::Inline => (self.manifest.shape[0], self.manifest.shape[1]),
            SectionAxis::Xline => (self.manifest.shape[1], self.manifest.shape[0]),
        };
        if index >= axis_len {
            return Err(SeisRefineError::InvalidSectionIndex {
                index,
                len: axis_len,
            });
        }

        let volume = load_array(self)?;
        let amplitudes = match axis {
            SectionAxis::Inline => volume.index_axis(Axis(0), index).to_owned(),
            SectionAxis::Xline => volume.index_axis(Axis(1), index).to_owned(),
        };
        let amplitude_values = amplitudes.into_raw_vec();
        let horizontal_axis = match axis {
            SectionAxis::Inline => self.manifest.axes.xlines.clone(),
            SectionAxis::Xline => self.manifest.axes.ilines.clone(),
        };
        let coordinate_value = match axis {
            SectionAxis::Inline => self.manifest.axes.ilines[index],
            SectionAxis::Xline => self.manifest.axes.xlines[index],
        };

        Ok(SectionView {
            dataset_id: self.dataset_id(),
            axis,
            coordinate: SectionCoordinate {
                index,
                value: coordinate_value,
            },
            traces,
            samples: self.manifest.shape[2],
            horizontal_axis_f64le: f64_vec_to_le_bytes(&horizontal_axis),
            sample_axis_f32le: f32_vec_to_le_bytes(&self.manifest.axes.sample_axis_ms),
            amplitudes_f32le: f32_vec_to_le_bytes(&amplitude_values),
            units: Some(SectionUnits {
                horizontal: Some(match axis {
                    SectionAxis::Inline => "xline".to_string(),
                    SectionAxis::Xline => "inline".to_string(),
                }),
                sample: Some("ms".to_string()),
                amplitude: Some("amplitude".to_string()),
            }),
            metadata: Some(SectionMetadata {
                store_id: Some(self.dataset_id().0),
                derived_from: self
                    .manifest
                    .derived_from
                    .as_ref()
                    .map(|derived| derived.parent_store.to_string_lossy().into_owned()),
                notes: vec![format!("kind:{:?}", self.manifest.kind)],
            }),
            display_defaults: Some(SectionDisplayDefaults {
                gain: 1.0,
                clip_min: None,
                clip_max: None,
                render_mode: SectionRenderMode::Heatmap,
                colormap: SectionColorMap::Grayscale,
                polarity: SectionPolarity::Normal,
            }),
        })
    }
}

pub fn create_store(
    root: impl AsRef<Path>,
    mut manifest: StoreManifest,
    data: &Array3<f32>,
    occupancy: Option<&Array2<u8>>,
) -> Result<StoreHandle, SeisRefineError> {
    let root = root.as_ref().to_path_buf();
    if root.exists() {
        return Err(SeisRefineError::StoreAlreadyExists(root));
    }

    fs::create_dir_all(&root)?;
    manifest.array_path = ARRAY_PATH.to_string();
    manifest.occupancy_array_path = occupancy.map(|_| OCCUPANCY_PATH.to_string());

    let store: ReadableWritableListableStorage = Arc::new(
        FilesystemStore::new(&root).map_err(|error| SeisRefineError::Message(error.to_string()))?,
    );
    GroupBuilder::new()
        .attributes(
            json!({
                "producer": "seis-runtime",
                "manifest": StoreManifest::FILE_NAME,
            })
            .as_object()
            .expect("object literal")
            .clone(),
        )
        .build(store.clone(), "/")
        .map_err(|error| SeisRefineError::Message(error.to_string()))?
        .store_metadata()?;

    let array = ArrayBuilder::new(
        manifest
            .shape
            .iter()
            .map(|value| *value as u64)
            .collect::<Vec<_>>(),
        manifest
            .chunk_shape
            .iter()
            .map(|value| *value as u64)
            .collect::<Vec<_>>(),
        data_type::float32(),
        0.0f32,
    )
    .dimension_names(["iline", "xline", "sample"].into())
    .build(store.clone(), ARRAY_PATH)
    .map_err(|error| SeisRefineError::Message(error.to_string()))?;
    array.store_metadata()?;
    array.store_array_subset(
        &[
            0_u64..manifest.shape[0] as u64,
            0_u64..manifest.shape[1] as u64,
            0_u64..manifest.shape[2] as u64,
        ],
        data.to_owned(),
    )?;

    if let Some(occupancy) = occupancy {
        let occupancy_array = ArrayBuilder::new(
            vec![manifest.shape[0] as u64, manifest.shape[1] as u64],
            vec![
                manifest.chunk_shape[0] as u64,
                manifest.chunk_shape[1] as u64,
            ],
            data_type::uint8(),
            0_u8,
        )
        .dimension_names(["iline", "xline"].into())
        .build(store, OCCUPANCY_PATH)
        .map_err(|error| SeisRefineError::Message(error.to_string()))?;
        occupancy_array.store_metadata()?;
        occupancy_array.store_array_subset(
            &[
                0_u64..manifest.shape[0] as u64,
                0_u64..manifest.shape[1] as u64,
            ],
            occupancy.to_owned(),
        )?;
    }

    fs::write(
        root.join(StoreManifest::FILE_NAME),
        serde_json::to_vec_pretty(&manifest)?,
    )?;

    Ok(StoreHandle { root, manifest })
}

pub fn open_store(root: impl AsRef<Path>) -> Result<StoreHandle, SeisRefineError> {
    let root = root.as_ref().to_path_buf();
    let manifest_path = root.join(StoreManifest::FILE_NAME);
    if !manifest_path.exists() {
        return Err(SeisRefineError::MissingManifest(manifest_path));
    }

    let manifest = serde_json::from_slice::<StoreManifest>(&fs::read(&manifest_path)?)?;
    Ok(StoreHandle { root, manifest })
}

pub fn describe_store(root: impl AsRef<Path>) -> Result<VolumeDescriptor, SeisRefineError> {
    Ok(open_store(root)?.volume_descriptor())
}

pub fn section_view(
    root: impl AsRef<Path>,
    axis: SectionAxis,
    index: usize,
) -> Result<SectionView, SeisRefineError> {
    open_store(root)?.section_view(axis, index)
}

pub fn load_array(handle: &StoreHandle) -> Result<Array3<f32>, SeisRefineError> {
    let store: ReadableWritableListableStorage = Arc::new(
        FilesystemStore::new(&handle.root)
            .map_err(|error| SeisRefineError::Message(error.to_string()))?,
    );
    let array = Array::open(store, &handle.manifest.array_path)
        .map_err(|error| SeisRefineError::Message(error.to_string()))?;
    let [ilines, xlines, samples] = handle.manifest.shape;
    let data = array.retrieve_array_subset::<ndarray::ArrayD<f32>>(&[
        0_u64..ilines as u64,
        0_u64..xlines as u64,
        0_u64..samples as u64,
    ])?;
    Ok(data.into_dimensionality::<Ix3>()?)
}

pub fn load_occupancy(handle: &StoreHandle) -> Result<Option<Array2<u8>>, SeisRefineError> {
    let Some(path) = handle.manifest.occupancy_array_path.as_deref() else {
        return Ok(None);
    };

    let store: ReadableWritableListableStorage = Arc::new(
        FilesystemStore::new(&handle.root)
            .map_err(|error| SeisRefineError::Message(error.to_string()))?,
    );
    let array =
        Array::open(store, path).map_err(|error| SeisRefineError::Message(error.to_string()))?;
    let [ilines, xlines, _] = handle.manifest.shape;
    let data = array
        .retrieve_array_subset::<ndarray::ArrayD<u8>>(&[
            0_u64..ilines as u64,
            0_u64..xlines as u64,
        ])?
        .into_dimensionality::<Ix2>()?;
    Ok(Some(data))
}

fn dataset_leaf_name(root: &Path) -> String {
    let raw = root.to_string_lossy();
    raw.rsplit(['/', '\\'])
        .find(|segment| !segment.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| raw.into_owned())
}

fn dataset_id_string(root: &Path) -> String {
    dataset_leaf_name(root)
}

fn dataset_label(root: &Path) -> String {
    Path::new(&dataset_leaf_name(root))
        .file_stem()
        .and_then(|value| value.to_str())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| dataset_id_string(root))
}

fn f64_vec_to_le_bytes(values: &[f64]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len() * std::mem::size_of::<f64>());
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn f32_vec_to_le_bytes(values: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len() * std::mem::size_of::<f32>());
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::metadata::{
        DatasetKind, GeometryProvenance, HeaderFieldSpec, SourceIdentity, StoreManifest, VolumeAxes,
    };
    use ndarray::Array3;
    use tempfile::tempdir;

    fn fixture_handle(root: &Path) -> StoreHandle {
        StoreHandle {
            root: root.to_path_buf(),
            manifest: StoreManifest {
                version: 1,
                kind: DatasetKind::Source,
                source: SourceIdentity {
                    source_path: PathBuf::from("input.sgy"),
                    file_size: 1,
                    trace_count: 1,
                    samples_per_trace: 6,
                    sample_interval_us: 2000,
                    sample_format_code: 5,
                    geometry: GeometryProvenance {
                        inline_field: HeaderFieldSpec {
                            name: "INLINE_3D".to_string(),
                            start_byte: 189,
                            value_type: "I32".to_string(),
                        },
                        crossline_field: HeaderFieldSpec {
                            name: "CROSSLINE_3D".to_string(),
                            start_byte: 193,
                            value_type: "I32".to_string(),
                        },
                        third_axis_field: None,
                    },
                    regularization: None,
                },
                shape: [3, 4, 6],
                chunk_shape: [2, 2, 6],
                axes: VolumeAxes {
                    ilines: vec![100.0, 101.0, 102.0],
                    xlines: vec![200.0, 201.0, 202.0, 203.0],
                    sample_axis_ms: vec![0.0, 2.0, 4.0, 6.0, 8.0, 10.0],
                },
                array_path: "/amplitude".to_string(),
                occupancy_array_path: None,
                created_by: "test".to_string(),
                derived_from: None,
            },
        }
    }

    #[test]
    fn volume_descriptor_uses_shared_domain_type() {
        let handle = fixture_handle(Path::new("C:\\data\\survey.zarr"));
        let descriptor = handle.volume_descriptor();

        assert_eq!(descriptor.id.0, "survey.zarr");
        assert_eq!(descriptor.label, "survey");
        assert_eq!(descriptor.shape, [3, 4, 6]);
        assert_eq!(descriptor.chunk_shape, [2, 2, 6]);
        assert_eq!(descriptor.sample_interval_ms, 2.0);
    }

    #[test]
    fn section_view_uses_shared_view_type() {
        let temp_dir = tempdir().expect("temp dir");
        let root = temp_dir.path().join("survey.zarr");
        let manifest = fixture_handle(Path::new("C:\\data\\survey.zarr")).manifest;
        let data = Array3::from_shape_fn((3, 4, 6), |(iline, xline, sample)| {
            iline as f32 * 100.0 + xline as f32 * 10.0 + sample as f32
        });
        create_store(&root, manifest, &data, None).expect("store should be created");
        let handle = open_store(&root).expect("store should open");
        let view = handle
            .section_view(SectionAxis::Inline, 1)
            .expect("inline section should be valid");

        assert_eq!(view.dataset_id.0, "survey.zarr");
        assert_eq!(view.axis, SectionAxis::Inline);
        assert_eq!(view.coordinate.index, 1);
        assert_eq!(view.coordinate.value, 101.0);
        assert_eq!(view.traces, 4);
        assert_eq!(view.samples, 6);
        assert_eq!(view.horizontal_axis_f64le.len(), 4 * 8);
        assert_eq!(view.sample_axis_f32le.len(), 6 * 4);
        assert_eq!(view.amplitudes_f32le.len(), 4 * 6 * 4);

        fs::remove_dir_all(&root).expect("temp store should be removable");
    }
}

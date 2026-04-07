// Local OpenVDS benchmark runner for the same synthetic trace-local workload
// used by the Rust compute-storage benchmark. Build against a local OpenVDS
// SDK build, for example:
// clang++ -std=c++17 -O3 scripts/openvds_storage_bench.cpp \
//   -I/tmp/open-vds-official/src/OpenVDS -I/tmp/open-vds-official/build/src/OpenVDS \
//   -L/tmp/open-vds-official/build/src/OpenVDS -lopenvds \
//   -Wl,-rpath,/tmp/open-vds-official/build/src/OpenVDS \
//   -o tmp/openvds_storage_bench

#include <OpenVDS/MetadataContainer.h>
#include <OpenVDS/OpenVDS.h>
#include <OpenVDS/VolumeDataAccess.h>
#include <OpenVDS/VolumeDataAxisDescriptor.h>
#include <OpenVDS/VolumeDataChannelDescriptor.h>
#include <OpenVDS/GlobalMetadataCommon.h>
#include <OpenVDS/KnownMetadata.h>
#include <OpenVDS/VolumeDataLayout.h>
#include <OpenVDS/VolumeDataLayoutDescriptor.h>
#include <OpenVDS/VolumeIndexer.h>

#include <algorithm>
#include <chrono>
#include <cmath>
#include <cstdint>
#include <filesystem>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <numeric>
#include <stdexcept>
#include <string>
#include <vector>

namespace fs = std::filesystem;

struct BenchResult {
  std::string dataset_name;
  int ilines;
  int xlines;
  int samples;
  int brick_size;
  std::uintmax_t input_bytes;
  std::uintmax_t input_file_count;
  double inline_section_read_ms;
  double xline_section_read_ms;
  double preview_pipeline_ms;
  double apply_pipeline_ms;
  std::uintmax_t output_bytes;
  std::uintmax_t output_file_count;
};

static float synthetic_value(int iline, int xline, int sample, int ilines, int xlines, int samples) {
  const float il = static_cast<float>(iline) / static_cast<float>(std::max(ilines, 1));
  const float xl = static_cast<float>(xline) / static_cast<float>(std::max(xlines, 1));
  const float smp = static_cast<float>(sample) / static_cast<float>(std::max(samples, 1));
  return ((std::sin(il * 17.0f) + std::cos(xl * 11.0f)) * (1.0f - smp)) +
         (std::sin(smp * 31.0f) * 0.35f);
}

static void apply_pipeline_to_traces(std::vector<float> &values, int trace_count, int samples, float scalar_factor) {
  for (float &value : values) {
    value *= scalar_factor;
  }

  for (int trace = 0; trace < trace_count; ++trace) {
    float energy = 0.0f;
    const int base = trace * samples;
    for (int sample = 0; sample < samples; ++sample) {
      const float value = values[base + sample];
      energy += value * value;
    }
    const float rms = std::sqrt(energy / static_cast<float>(std::max(samples, 1)));
    const float divisor = std::max(rms, 1.0e-12f);
    for (int sample = 0; sample < samples; ++sample) {
      values[base + sample] /= divisor;
    }
  }
}

static OpenVDS::VolumeDataLayoutDescriptor::BrickSize brick_size_enum(int brick_size) {
  switch (brick_size) {
  case 32:
    return OpenVDS::VolumeDataLayoutDescriptor::BrickSize_32;
  case 64:
    return OpenVDS::VolumeDataLayoutDescriptor::BrickSize_64;
  case 128:
    return OpenVDS::VolumeDataLayoutDescriptor::BrickSize_128;
  case 256:
    return OpenVDS::VolumeDataLayoutDescriptor::BrickSize_256;
  default:
    throw std::runtime_error("Unsupported brick size");
  }
}

static std::uintmax_t file_size_or_zero(const fs::path &path) {
  std::error_code ec;
  const auto size = fs::file_size(path, ec);
  return ec ? 0 : size;
}

static std::pair<std::uintmax_t, std::uintmax_t> path_metrics(const fs::path &path) {
  if (!fs::exists(path)) {
    return {0, 0};
  }
  if (fs::is_regular_file(path)) {
    return {file_size_or_zero(path), 1};
  }

  std::uintmax_t bytes = 0;
  std::uintmax_t files = 0;
  for (const auto &entry : fs::recursive_directory_iterator(path)) {
    if (entry.is_regular_file()) {
      bytes += file_size_or_zero(entry.path());
      files += 1;
    }
  }
  return {bytes, files};
}

static void ensure_ok(const OpenVDS::Error &error, const char *context) {
  if (error.code) {
    throw std::runtime_error(std::string(context) + ": " + error.string);
  }
}

static std::vector<float> request_subset(
    OpenVDS::VolumeDataAccessManager &access_manager,
    const int (&min)[OpenVDS::Dimensionality_Max],
    const int (&max)[OpenVDS::Dimensionality_Max]) {
  const int samples = max[0] - min[0];
  const int xlines = max[1] - min[1];
  const int ilines = max[2] - min[2];
  std::vector<float> values(static_cast<std::size_t>(samples) * static_cast<std::size_t>(xlines) *
                            static_cast<std::size_t>(ilines));
  auto request = access_manager.RequestVolumeSubset<float>(
      values.data(), values.size() * sizeof(float), OpenVDS::Dimensions_012, 0, 0, min, max);
  request->WaitForCompletion();
  return values;
}

static OpenVDS::VDS *create_file_vds(
    const fs::path &path,
    int ilines,
    int xlines,
    int samples,
    int brick_size,
    OpenVDS::CompressionMethod compression_method) {
  OpenVDS::MetadataContainer metadata;
  std::vector<OpenVDS::VolumeDataAxisDescriptor> axes;
  axes.emplace_back(samples, KNOWNMETADATA_SURVEYCOORDINATE_INLINECROSSLINE_AXISNAME_SAMPLE, "ms", 0.0f,
                    static_cast<float>(std::max(samples - 1, 0)) * 2.0f);
  axes.emplace_back(xlines, KNOWNMETADATA_SURVEYCOORDINATE_INLINECROSSLINE_AXISNAME_CROSSLINE, "",
                    0.0f, static_cast<float>(std::max(xlines - 1, 0)));
  axes.emplace_back(ilines, KNOWNMETADATA_SURVEYCOORDINATE_INLINECROSSLINE_AXISNAME_INLINE, "",
                    0.0f, static_cast<float>(std::max(ilines - 1, 0)));

  std::vector<OpenVDS::VolumeDataChannelDescriptor> channels;
  channels.emplace_back(
      OpenVDS::VolumeDataChannelDescriptor::Format_R32,
      OpenVDS::VolumeDataChannelDescriptor::Components_1,
      AMPLITUDE_ATTRIBUTE_NAME,
      "",
      -4.0f,
      4.0f,
      OpenVDS::VolumeDataMapping::Direct,
      1,
      OpenVDS::VolumeDataChannelDescriptor::Default,
      0.0f,
      1.0f,
      0.0f);

  const auto layout = OpenVDS::VolumeDataLayoutDescriptor(
      brick_size_enum(brick_size),
      0,
      0,
      1,
      OpenVDS::VolumeDataLayoutDescriptor::LODLevels_None,
      OpenVDS::VolumeDataLayoutDescriptor::Options_None);

  OpenVDS::Error error;
  OpenVDS::VDSFileOpenOptions options(path.string());
  auto *vds = OpenVDS::Create(options, layout, axes, channels, metadata, compression_method, 0.0f, error);
  ensure_ok(error, "OpenVDS::Create");
  return vds;
}

static void fill_synthetic_volume(OpenVDS::VDS *vds, int ilines, int xlines, int samples) {
  auto access_manager = OpenVDS::GetAccessManager(vds);
  auto page_accessor =
      access_manager.CreateVolumeDataPageAccessor(OpenVDS::Dimensions_012, 0, 0, 64,
                                                  OpenVDS::VolumeDataAccessManager::AccessMode_CreateWithoutLODGeneration);

  const auto *layout = page_accessor->GetLayout();
  const auto channel_format = layout->GetChannelFormat(page_accessor->GetChannelIndex());
  if (channel_format != OpenVDS::VolumeDataFormat::Format_R32) {
    throw std::runtime_error("Synthetic writer expects R32");
  }

  const auto chunk_count = static_cast<int>(page_accessor->GetChunkCount());
  for (int chunk = 0; chunk < chunk_count; ++chunk) {
    OpenVDS::VolumeDataPage *page = page_accessor->CreatePage(chunk);
    OpenVDS::VolumeIndexer3D indexer(page, 0, 0, OpenVDS::Dimensions_012, layout);
    int pitch[OpenVDS::Dimensionality_Max] = {};
    auto *buffer = static_cast<float *>(page->GetWritableBuffer(pitch));
    for (int z = 0; z < indexer.dataBlockSamples[2]; ++z) {
      for (int y = 0; y < indexer.dataBlockSamples[1]; ++y) {
        for (int x = 0; x < indexer.dataBlockSamples[0]; ++x) {
          const auto voxel = indexer.LocalIndexToVoxelIndex(OpenVDS::IntVector3{x, y, z});
          const int sample = voxel[0];
          const int xline = voxel[1];
          const int iline = voxel[2];
          buffer[indexer.LocalIndexToDataIndex(OpenVDS::IntVector3{x, y, z})] =
              synthetic_value(iline, xline, sample, ilines, xlines, samples);
        }
      }
    }
    page->Release();
  }

  page_accessor->Commit();
  OpenVDS::Error error;
  access_manager.Flush(error);
  ensure_ok(error, "VolumeDataAccessManager::Flush");
}

static void write_chunk_page(OpenVDS::VolumeDataPage *page, const std::vector<float> &values,
                             const OpenVDS::VolumeIndexer3D &indexer) {
  int pitch[OpenVDS::Dimensionality_Max] = {};
  auto *buffer = static_cast<float *>(page->GetWritableBuffer(pitch));
  for (int z = 0; z < indexer.dataBlockSamples[2]; ++z) {
    for (int y = 0; y < indexer.dataBlockSamples[1]; ++y) {
      for (int x = 0; x < indexer.dataBlockSamples[0]; ++x) {
        const int idx = indexer.LocalIndexToDataIndex(OpenVDS::IntVector3{x, y, z});
        buffer[idx] = values[static_cast<std::size_t>(idx)];
      }
    }
  }
}

static BenchResult benchmark_openvds_dataset(const std::string &dataset_name, int ilines, int xlines,
                                             int samples, int brick_size, float scalar_factor,
                                             const fs::path &input_path, bool create_input) {
  if (fs::exists(input_path)) {
    fs::remove(input_path);
  }

  OpenVDS::VDS *input_vds = nullptr;
  if (create_input) {
    input_vds = create_file_vds(input_path, ilines, xlines, samples, brick_size, OpenVDS::CompressionMethod::None);
    fill_synthetic_volume(input_vds, ilines, xlines, samples);
    OpenVDS::Close(input_vds);
    input_vds = nullptr;
  }

  OpenVDS::Error error;
  input_vds = OpenVDS::Open(OpenVDS::VDSFileOpenOptions(input_path.string()), error);
  ensure_ok(error, "OpenVDS::Open");
  auto access_manager = OpenVDS::GetAccessManager(input_vds);

  const int mid_inline = ilines / 2;
  const int mid_xline = xlines / 2;
  const int inline_min[OpenVDS::Dimensionality_Max] = {0, 0, mid_inline, 0, 0, 0};
  const int inline_max[OpenVDS::Dimensionality_Max] = {samples, xlines, mid_inline + 1, 1, 1, 1};
  const int xline_min[OpenVDS::Dimensionality_Max] = {0, mid_xline, 0, 0, 0, 0};
  const int xline_max[OpenVDS::Dimensionality_Max] = {samples, mid_xline + 1, ilines, 1, 1, 1};

  const auto inline_started = std::chrono::steady_clock::now();
  auto inline_section = request_subset(access_manager, inline_min, inline_max);
  const auto inline_section_read_ms =
      std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now() - inline_started).count();

  const auto xline_started = std::chrono::steady_clock::now();
  auto xline_section = request_subset(access_manager, xline_min, xline_max);
  const auto xline_section_read_ms =
      std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now() - xline_started).count();

  (void)xline_section;

  const auto preview_started = std::chrono::steady_clock::now();
  auto preview_section = request_subset(access_manager, inline_min, inline_max);
  apply_pipeline_to_traces(preview_section, xlines, samples, scalar_factor);
  const auto preview_pipeline_ms =
      std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now() - preview_started).count();

  const fs::path output_path =
      input_path.parent_path() / (input_path.stem().string() + ".pipeline.vds");
  if (fs::exists(output_path)) {
    fs::remove(output_path);
  }
  auto *output_vds = create_file_vds(output_path, ilines, xlines, samples, brick_size, OpenVDS::CompressionMethod::None);
  auto output_access_manager = OpenVDS::GetAccessManager(output_vds);
  auto output_page_accessor =
      output_access_manager.CreateVolumeDataPageAccessor(OpenVDS::Dimensions_012, 0, 0, 64,
                                                         OpenVDS::VolumeDataAccessManager::AccessMode_CreateWithoutLODGeneration);
  const auto *output_layout = output_page_accessor->GetLayout();
  const auto output_chunk_count = static_cast<int>(output_page_accessor->GetChunkCount());

  const auto apply_started = std::chrono::steady_clock::now();
  for (int chunk = 0; chunk < output_chunk_count; ++chunk) {
    OpenVDS::VolumeDataPage *page = output_page_accessor->CreatePage(chunk);
    OpenVDS::VolumeIndexer3D indexer(page, 0, 0, OpenVDS::Dimensions_012, output_layout);
    int min[OpenVDS::Dimensionality_Max] = {};
    int max[OpenVDS::Dimensionality_Max] = {};
    for (int dimension = 0; dimension < OpenVDS::Dimensionality_Max; ++dimension) {
      min[dimension] = indexer.voxelMin[dimension];
      max[dimension] = indexer.voxelMax[dimension];
    }
    auto chunk_values = request_subset(access_manager, min, max);
    const int trace_count = (max[2] - min[2]) * (max[1] - min[1]);
    apply_pipeline_to_traces(chunk_values, trace_count, max[0] - min[0], scalar_factor);
    write_chunk_page(page, chunk_values, indexer);
    page->Release();
  }
  output_page_accessor->Commit();
  output_access_manager.Flush(error);
  ensure_ok(error, "Output flush");
  const auto apply_pipeline_ms =
      std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now() - apply_started).count();

  OpenVDS::Close(output_vds);
  OpenVDS::Close(input_vds);

  const auto [input_bytes, input_files] = path_metrics(input_path);
  const auto [output_bytes, output_files] = path_metrics(output_path);

  return BenchResult{
      dataset_name,
      ilines,
      xlines,
      samples,
      brick_size,
      input_bytes,
      input_files,
      inline_section_read_ms,
      xline_section_read_ms,
      preview_pipeline_ms,
      apply_pipeline_ms,
      output_bytes,
      output_files,
  };
}

static void print_json(const BenchResult &result) {
  std::cout << std::fixed << std::setprecision(3);
  std::cout << "{\n"
            << "  \"dataset_name\": \"" << result.dataset_name << "\",\n"
            << "  \"shape\": [" << result.ilines << ", " << result.xlines << ", " << result.samples << "],\n"
            << "  \"brick_size\": " << result.brick_size << ",\n"
            << "  \"input_store_bytes\": " << result.input_bytes << ",\n"
            << "  \"input_file_count\": " << result.input_file_count << ",\n"
            << "  \"inline_section_read_ms\": " << result.inline_section_read_ms << ",\n"
            << "  \"xline_section_read_ms\": " << result.xline_section_read_ms << ",\n"
            << "  \"preview_pipeline_ms\": " << result.preview_pipeline_ms << ",\n"
            << "  \"apply_pipeline_ms\": " << result.apply_pipeline_ms << ",\n"
            << "  \"pipeline_output_bytes\": " << result.output_bytes << ",\n"
            << "  \"pipeline_output_file_count\": " << result.output_file_count << "\n"
            << "}\n";
}

int main(int argc, char **argv) {
  if (argc != 7) {
    std::cerr << "usage: openvds_storage_bench <dataset-name> <ilines> <xlines> <samples> <brick-size> <output-dir>\n";
    return 1;
  }

  const std::string dataset_name = argv[1];
  const int ilines = std::stoi(argv[2]);
  const int xlines = std::stoi(argv[3]);
  const int samples = std::stoi(argv[4]);
  const int brick_size = std::stoi(argv[5]);
  const fs::path output_dir = argv[6];
  fs::create_directories(output_dir);

  const fs::path input_path = output_dir / (dataset_name + ".vds");
  try {
    const auto result = benchmark_openvds_dataset(dataset_name, ilines, xlines, samples, brick_size, 2.0f, input_path, true);
    print_json(result);
    return 0;
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 2;
  }
}

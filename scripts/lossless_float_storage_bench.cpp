// External exact-lossless benchmark runner for floating-point compression
// candidates. This is an archive-tier comparison harness intended to answer:
// how much exact storage reduction and decode/apply cost do we see for
// specialist float-aware codecs versus pragmatic byte-filter baselines?
//
// Build against local fpzip and c-blosc2 builds, for example:
// cl /std:c++17 /EHsc /O2 scripts/lossless_float_storage_bench.cpp ^
//   /I C:\path\to\fpzip\include ^
//   /I C:\path\to\c-blosc2\include ^
//   /Fe:tmp\lossless_float_storage_bench.exe ^
//   /link /LIBPATH:C:\path\to\fpzip\build\lib fpzip.lib ^
//         /LIBPATH:C:\path\to\c-blosc2\build\blosc blosc2.lib

#ifndef NOMINMAX
#define NOMINMAX
#endif

#include <blosc2.h>
#include <fpzip.h>

#include <algorithm>
#include <chrono>
#include <cmath>
#include <cstdint>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <memory>
#include <regex>
#include <stdexcept>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

namespace fs = std::filesystem;

struct BenchResult {
  std::string dataset_name;
  std::string codec;
  int ilines;
  int xlines;
  int samples;
  int compression_level;
  std::uintmax_t input_store_bytes;
  std::uintmax_t compressed_bytes;
  double compression_ratio;
  double compression_ms;
  double decompression_ms;
  double decode_inline_section_ms;
  double decode_xline_section_ms;
  double decode_preview_pipeline_ms;
  double decode_apply_pipeline_ms;
  bool exact_roundtrip;
};

struct CodecSpec {
  std::string label;
  int compression_level;
  virtual ~CodecSpec() = default;
  virtual std::vector<std::uint8_t> compress(const std::vector<float> &input,
                                             int ilines,
                                             int xlines,
                                             int samples) const = 0;
  virtual std::vector<float> decompress(const std::vector<std::uint8_t> &input,
                                        int ilines,
                                        int xlines,
                                        int samples) const = 0;
};

struct VolumeData {
  std::string dataset_name;
  int ilines;
  int xlines;
  int samples;
  int tile_ilines;
  int tile_xlines;
  int tile_samples;
  std::vector<float> values;
};

static float synthetic_value(int iline, int xline, int sample, int ilines, int xlines, int samples) {
  const float il = static_cast<float>(iline) / static_cast<float>(std::max(ilines, 1));
  const float xl = static_cast<float>(xline) / static_cast<float>(std::max(xlines, 1));
  const float smp = static_cast<float>(sample) / static_cast<float>(std::max(samples, 1));
  return ((std::sin(il * 17.0f) + std::cos(xl * 11.0f)) * (1.0f - smp)) +
         (std::sin(smp * 31.0f) * 0.35f);
}

static std::vector<float> make_synthetic_volume(int ilines, int xlines, int samples) {
  std::vector<float> values(static_cast<std::size_t>(ilines) * static_cast<std::size_t>(xlines) *
                            static_cast<std::size_t>(samples));
  std::size_t index = 0;
  for (int iline = 0; iline < ilines; ++iline) {
    for (int xline = 0; xline < xlines; ++xline) {
      for (int sample = 0; sample < samples; ++sample) {
        values[index++] = synthetic_value(iline, xline, sample, ilines, xlines, samples);
      }
    }
  }
  return values;
}

static std::string read_text_file(const fs::path &path) {
  std::ifstream stream(path, std::ios::binary);
  if (!stream) {
    throw std::runtime_error("Failed to open text file: " + path.string());
  }
  return std::string(std::istreambuf_iterator<char>(stream), std::istreambuf_iterator<char>());
}

static std::vector<float> read_float_file(const fs::path &path, std::size_t expected_values) {
  std::ifstream stream(path, std::ios::binary);
  if (!stream) {
    throw std::runtime_error("Failed to open float file: " + path.string());
  }

  stream.seekg(0, std::ios::end);
  const auto file_size = static_cast<std::size_t>(stream.tellg());
  stream.seekg(0, std::ios::beg);
  if (file_size != expected_values * sizeof(float)) {
    throw std::runtime_error("Unexpected amplitude.bin size for " + path.string());
  }

  std::vector<float> values(expected_values);
  stream.read(reinterpret_cast<char *>(values.data()), static_cast<std::streamsize>(file_size));
  if (!stream) {
    throw std::runtime_error("Failed to read float file: " + path.string());
  }
  return values;
}

static VolumeData load_tbvol_volume(const fs::path &tbvol_dir) {
  const fs::path manifest_path = tbvol_dir / "manifest.json";
  const fs::path amplitude_path = tbvol_dir / "amplitude.bin";
  const std::string manifest = read_text_file(manifest_path);

  const std::regex shape_pattern(R"("shape"\s*:\s*\[\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\])");
  const std::regex tile_shape_pattern(R"("tile_shape"\s*:\s*\[\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\])");
  std::smatch match;
  if (!std::regex_search(manifest, match, shape_pattern)) {
    throw std::runtime_error("Failed to parse volume shape from " + manifest_path.string());
  }

  const int ilines = std::stoi(match[1].str());
  const int xlines = std::stoi(match[2].str());
  const int samples = std::stoi(match[3].str());
  if (!std::regex_search(manifest, match, tile_shape_pattern)) {
    throw std::runtime_error("Failed to parse tile shape from " + manifest_path.string());
  }
  const int tile_ilines = std::stoi(match[1].str());
  const int tile_xlines = std::stoi(match[2].str());
  const int tile_samples = std::stoi(match[3].str());

  if (manifest.find("\"sample_type\": \"f32\"") == std::string::npos) {
    throw std::runtime_error("Only f32 tbvol stores are supported by this benchmark");
  }
  if (manifest.find("\"endianness\": \"little\"") == std::string::npos) {
    throw std::runtime_error("Only little-endian tbvol stores are supported by this benchmark");
  }
  if (tile_samples != samples) {
    throw std::runtime_error("tbvol tiles must span the full sample axis");
  }

  const std::size_t value_count =
      static_cast<std::size_t>(ilines) * static_cast<std::size_t>(xlines) * static_cast<std::size_t>(samples);
  const int tile_grid_ilines = (ilines + tile_ilines - 1) / tile_ilines;
  const int tile_grid_xlines = (xlines + tile_xlines - 1) / tile_xlines;
  const std::size_t tile_value_count =
      static_cast<std::size_t>(tile_ilines) * static_cast<std::size_t>(tile_xlines) * static_cast<std::size_t>(samples);
  const std::size_t expected_tile_count =
      static_cast<std::size_t>(tile_grid_ilines) * static_cast<std::size_t>(tile_grid_xlines);

  std::ifstream stream(amplitude_path, std::ios::binary);
  if (!stream) {
    throw std::runtime_error("Failed to open float file: " + amplitude_path.string());
  }
  stream.seekg(0, std::ios::end);
  const auto file_size = static_cast<std::size_t>(stream.tellg());
  stream.seekg(0, std::ios::beg);
  if (file_size != expected_tile_count * tile_value_count * sizeof(float)) {
    throw std::runtime_error("Unexpected amplitude.bin size for " + amplitude_path.string());
  }

  std::vector<float> values(value_count, 0.0f);
  std::vector<float> tile(tile_value_count);
  for (int tile_i = 0; tile_i < tile_grid_ilines; ++tile_i) {
    for (int tile_x = 0; tile_x < tile_grid_xlines; ++tile_x) {
      stream.read(reinterpret_cast<char *>(tile.data()),
                  static_cast<std::streamsize>(tile_value_count * sizeof(float)));
      if (!stream) {
        throw std::runtime_error("Failed to read tile data from " + amplitude_path.string());
      }

      const int origin_i = tile_i * tile_ilines;
      const int origin_x = tile_x * tile_xlines;
      const int effective_i = std::min(tile_ilines, ilines - origin_i);
      const int effective_x = std::min(tile_xlines, xlines - origin_x);
      for (int local_i = 0; local_i < effective_i; ++local_i) {
        for (int local_x = 0; local_x < effective_x; ++local_x) {
          const std::size_t src_trace =
              (static_cast<std::size_t>(local_i) * static_cast<std::size_t>(tile_xlines)) + static_cast<std::size_t>(local_x);
          const std::size_t dst_trace =
              (static_cast<std::size_t>(origin_i + local_i) * static_cast<std::size_t>(xlines)) +
              static_cast<std::size_t>(origin_x + local_x);
          std::memcpy(values.data() + (dst_trace * static_cast<std::size_t>(samples)),
                      tile.data() + (src_trace * static_cast<std::size_t>(samples)),
                      static_cast<std::size_t>(samples) * sizeof(float));
        }
      }
    }
  }

  return VolumeData{
      tbvol_dir.stem().string(),
      ilines,
      xlines,
      samples,
      tile_ilines,
      tile_xlines,
      tile_samples,
      std::move(values),
  };
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

static std::vector<float> extract_inline_section(const std::vector<float> &values,
                                                 int ilines,
                                                 int xlines,
                                                 int samples,
                                                 int iline_index) {
  std::vector<float> section(static_cast<std::size_t>(xlines) * static_cast<std::size_t>(samples));
  std::size_t out = 0;
  for (int xline = 0; xline < xlines; ++xline) {
    const std::size_t base = (static_cast<std::size_t>(iline_index) * static_cast<std::size_t>(xlines) +
                              static_cast<std::size_t>(xline)) *
                             static_cast<std::size_t>(samples);
    std::copy_n(values.begin() + static_cast<std::ptrdiff_t>(base), samples,
                section.begin() + static_cast<std::ptrdiff_t>(out));
    out += static_cast<std::size_t>(samples);
  }
  return section;
}

static std::vector<float> extract_xline_section(const std::vector<float> &values,
                                                int ilines,
                                                int xlines,
                                                int samples,
                                                int xline_index) {
  std::vector<float> section(static_cast<std::size_t>(ilines) * static_cast<std::size_t>(samples));
  std::size_t out = 0;
  for (int iline = 0; iline < ilines; ++iline) {
    const std::size_t base = (static_cast<std::size_t>(iline) * static_cast<std::size_t>(xlines) +
                              static_cast<std::size_t>(xline_index)) *
                             static_cast<std::size_t>(samples);
    std::copy_n(values.begin() + static_cast<std::ptrdiff_t>(base), samples,
                section.begin() + static_cast<std::ptrdiff_t>(out));
    out += static_cast<std::size_t>(samples);
  }
  return section;
}

static double elapsed_ms(std::chrono::steady_clock::time_point start) {
  return std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now() - start).count();
}

static void write_bytes(const fs::path &path, const std::vector<std::uint8_t> &bytes) {
  std::ofstream stream(path, std::ios::binary | std::ios::trunc);
  if (!stream) {
    throw std::runtime_error("Failed to open output file: " + path.string());
  }
  stream.write(reinterpret_cast<const char *>(bytes.data()),
               static_cast<std::streamsize>(bytes.size()));
  if (!stream) {
    throw std::runtime_error("Failed to write output file: " + path.string());
  }
}

static void append_u64_le(std::vector<std::uint8_t> &out, std::uint64_t value) {
  for (int i = 0; i < 8; ++i) {
    out.push_back(static_cast<std::uint8_t>((value >> (i * 8)) & 0xffu));
  }
}

static std::uint64_t read_u64_le(const std::vector<std::uint8_t> &input, std::size_t &offset) {
  if (offset + 8 > input.size()) {
    throw std::runtime_error("Corrupt chunked stream header");
  }
  std::uint64_t value = 0;
  for (int i = 0; i < 8; ++i) {
    value |= static_cast<std::uint64_t>(input[offset + static_cast<std::size_t>(i)]) << (i * 8);
  }
  offset += 8;
  return value;
}

static std::vector<float> gather_padded_tile(const std::vector<float> &input,
                                             int ilines,
                                             int xlines,
                                             int samples,
                                             int tile_i,
                                             int tile_x,
                                             int chunk_ilines,
                                             int chunk_xlines) {
  std::vector<float> tile(static_cast<std::size_t>(chunk_ilines) * static_cast<std::size_t>(chunk_xlines) *
                          static_cast<std::size_t>(samples), 0.0f);
  const int origin_i = tile_i * chunk_ilines;
  const int origin_x = tile_x * chunk_xlines;
  const int effective_i = std::min(chunk_ilines, ilines - origin_i);
  const int effective_x = std::min(chunk_xlines, xlines - origin_x);
  for (int local_i = 0; local_i < effective_i; ++local_i) {
    for (int local_x = 0; local_x < effective_x; ++local_x) {
      const std::size_t src_trace =
          (static_cast<std::size_t>(origin_i + local_i) * static_cast<std::size_t>(xlines)) +
          static_cast<std::size_t>(origin_x + local_x);
      const std::size_t dst_trace =
          (static_cast<std::size_t>(local_i) * static_cast<std::size_t>(chunk_xlines)) + static_cast<std::size_t>(local_x);
      std::memcpy(tile.data() + (dst_trace * static_cast<std::size_t>(samples)),
                  input.data() + (src_trace * static_cast<std::size_t>(samples)),
                  static_cast<std::size_t>(samples) * sizeof(float));
    }
  }
  return tile;
}

static void scatter_padded_tile(const std::vector<float> &tile,
                                int ilines,
                                int xlines,
                                int samples,
                                int tile_i,
                                int tile_x,
                                int chunk_ilines,
                                int chunk_xlines,
                                std::vector<float> &output) {
  const int origin_i = tile_i * chunk_ilines;
  const int origin_x = tile_x * chunk_xlines;
  const int effective_i = std::min(chunk_ilines, ilines - origin_i);
  const int effective_x = std::min(chunk_xlines, xlines - origin_x);
  for (int local_i = 0; local_i < effective_i; ++local_i) {
    for (int local_x = 0; local_x < effective_x; ++local_x) {
      const std::size_t src_trace =
          (static_cast<std::size_t>(local_i) * static_cast<std::size_t>(chunk_xlines)) + static_cast<std::size_t>(local_x);
      const std::size_t dst_trace =
          (static_cast<std::size_t>(origin_i + local_i) * static_cast<std::size_t>(xlines)) +
          static_cast<std::size_t>(origin_x + local_x);
      std::memcpy(output.data() + (dst_trace * static_cast<std::size_t>(samples)),
                  tile.data() + (src_trace * static_cast<std::size_t>(samples)),
                  static_cast<std::size_t>(samples) * sizeof(float));
    }
  }
}

static std::vector<std::uint32_t> encode_trace_xor(const std::vector<float> &input, int trace_count, int samples) {
  std::vector<std::uint32_t> encoded(input.size());
  for (int trace = 0; trace < trace_count; ++trace) {
    std::uint32_t previous_bits = 0;
    const std::size_t base = static_cast<std::size_t>(trace) * static_cast<std::size_t>(samples);
    for (int sample = 0; sample < samples; ++sample) {
      std::uint32_t bits = 0;
      const std::size_t index = base + static_cast<std::size_t>(sample);
      std::memcpy(&bits, &input[index], sizeof(bits));
      encoded[index] = bits ^ previous_bits;
      previous_bits = bits;
    }
  }
  return encoded;
}

static std::vector<float> decode_trace_xor(const std::vector<std::uint32_t> &encoded, int trace_count, int samples) {
  std::vector<float> output(encoded.size());
  for (int trace = 0; trace < trace_count; ++trace) {
    std::uint32_t previous_bits = 0;
    const std::size_t base = static_cast<std::size_t>(trace) * static_cast<std::size_t>(samples);
    for (int sample = 0; sample < samples; ++sample) {
      const std::size_t index = base + static_cast<std::size_t>(sample);
      const std::uint32_t bits = encoded[index] ^ previous_bits;
      std::memcpy(&output[index], &bits, sizeof(bits));
      previous_bits = bits;
    }
  }
  return output;
}

class FpzipCodec final : public CodecSpec {
public:
  explicit FpzipCodec(int compression_level) {
    label = "fpzip";
    this->compression_level = compression_level;
  }

  std::vector<std::uint8_t> compress(const std::vector<float> &input,
                                     int ilines,
                                     int xlines,
                                     int samples) const override {
    const std::size_t input_bytes = input.size() * sizeof(float);
    std::vector<std::uint8_t> output(input_bytes + 4096);

    FPZ *fpz = fpzip_write_to_buffer(output.data(), output.size());
    if (!fpz) {
      throw std::runtime_error("fpzip_write_to_buffer failed");
    }

    fpz->nx = samples;
    fpz->ny = xlines;
    fpz->nz = ilines;
    fpz->nf = 1;
    fpz->type = FPZIP_TYPE_FLOAT;
    fpz->prec = 0;

    if (!fpzip_write_header(fpz)) {
      fpzip_write_close(fpz);
      throw std::runtime_error("fpzip_write_header failed");
    }
    const std::size_t compressed_size = fpzip_write(fpz, input.data());
    fpzip_write_close(fpz);
    if (compressed_size == 0) {
      throw std::runtime_error("fpzip_write failed");
    }
    output.resize(compressed_size);
    return output;
  }

  std::vector<float> decompress(const std::vector<std::uint8_t> &input,
                                int ilines,
                                int xlines,
                                int samples) const override {
    std::vector<float> output(static_cast<std::size_t>(ilines) * static_cast<std::size_t>(xlines) *
                              static_cast<std::size_t>(samples));

    FPZ *fpz = fpzip_read_from_buffer(const_cast<void *>(static_cast<const void *>(input.data())));
    if (!fpz) {
      throw std::runtime_error("fpzip_read_from_buffer failed");
    }
    if (!fpzip_read_header(fpz)) {
      fpzip_read_close(fpz);
      throw std::runtime_error("fpzip_read_header failed");
    }
    const std::size_t read_count = fpzip_read(fpz, output.data());
    fpzip_read_close(fpz);
    if (read_count == 0) {
      throw std::runtime_error("fpzip_read failed");
    }
    return output;
  }
};

class BloscCodec final : public CodecSpec {
public:
  BloscCodec(std::string label, const char *compressor_name, int compression_level)
      : compressor_name_(compressor_name) {
    this->label = std::move(label);
    this->compression_level = compression_level;
  }

  std::vector<std::uint8_t> compress(const std::vector<float> &input,
                                     int /*ilines*/,
                                     int /*xlines*/,
                                     int /*samples*/) const override {
    if (blosc1_set_compressor(compressor_name_) < 0) {
      throw std::runtime_error("blosc1_set_compressor failed");
    }
    const auto input_bytes = static_cast<int32_t>(input.size() * sizeof(float));
    std::vector<std::uint8_t> output(static_cast<std::size_t>(input_bytes) + BLOSC2_MAX_OVERHEAD);
    const int compressed_size =
        blosc1_compress(compression_level, BLOSC_BITSHUFFLE, static_cast<int32_t>(sizeof(float)), input_bytes,
                        input.data(), output.data(), static_cast<int32_t>(output.size()));
    if (compressed_size <= 0) {
      throw std::runtime_error("blosc1_compress failed");
    }
    output.resize(static_cast<std::size_t>(compressed_size));
    return output;
  }

  std::vector<float> decompress(const std::vector<std::uint8_t> &input,
                                int ilines,
                                int xlines,
                                int samples) const override {
    std::vector<float> output(static_cast<std::size_t>(ilines) * static_cast<std::size_t>(xlines) *
                              static_cast<std::size_t>(samples));
    const int decompressed_size = blosc1_decompress(input.data(), output.data(),
                                                    static_cast<int32_t>(output.size() * sizeof(float)));
    if (decompressed_size < 0) {
      throw std::runtime_error("blosc1_decompress failed");
    }
    return output;
  }

private:
  const char *compressor_name_;
};

class TraceXorBloscCodec final : public CodecSpec {
public:
  TraceXorBloscCodec(std::string label, const char *compressor_name, int compression_level)
      : compressor_name_(compressor_name) {
    this->label = std::move(label);
    this->compression_level = compression_level;
  }

  std::vector<std::uint8_t> compress(const std::vector<float> &input,
                                     int ilines,
                                     int xlines,
                                     int samples) const override {
    if (blosc1_set_compressor(compressor_name_) < 0) {
      throw std::runtime_error("blosc1_set_compressor failed");
    }
    const int trace_count = ilines * xlines;
    const auto encoded = encode_trace_xor(input, trace_count, samples);
    const auto input_bytes = static_cast<int32_t>(encoded.size() * sizeof(std::uint32_t));
    std::vector<std::uint8_t> output(static_cast<std::size_t>(input_bytes) + BLOSC2_MAX_OVERHEAD);
    const int compressed_size =
        blosc1_compress(compression_level, BLOSC_BITSHUFFLE, static_cast<int32_t>(sizeof(std::uint32_t)), input_bytes,
                        encoded.data(), output.data(), static_cast<int32_t>(output.size()));
    if (compressed_size <= 0) {
      throw std::runtime_error("blosc1_compress failed");
    }
    output.resize(static_cast<std::size_t>(compressed_size));
    return output;
  }

  std::vector<float> decompress(const std::vector<std::uint8_t> &input,
                                int ilines,
                                int xlines,
                                int samples) const override {
    std::vector<std::uint32_t> encoded(static_cast<std::size_t>(ilines) * static_cast<std::size_t>(xlines) *
                                       static_cast<std::size_t>(samples));
    const int decompressed_size = blosc1_decompress(
        input.data(), encoded.data(), static_cast<int32_t>(encoded.size() * sizeof(std::uint32_t)));
    if (decompressed_size < 0) {
      throw std::runtime_error("blosc1_decompress failed");
    }
    return decode_trace_xor(encoded, ilines * xlines, samples);
  }

private:
  const char *compressor_name_;
};

class TileBloscCodec final : public CodecSpec {
public:
  TileBloscCodec(std::string label,
                 const char *compressor_name,
                 int compression_level,
                 int chunk_ilines,
                 int chunk_xlines,
                 int chunk_samples)
      : compressor_name_(compressor_name),
        chunk_ilines_(chunk_ilines),
        chunk_xlines_(chunk_xlines),
        chunk_samples_(chunk_samples) {
    this->label = std::move(label);
    this->compression_level = compression_level;
  }

  std::vector<std::uint8_t> compress(const std::vector<float> &input,
                                     int ilines,
                                     int xlines,
                                     int samples) const override {
    if (chunk_samples_ != samples) {
      throw std::runtime_error("TileBloscCodec expects chunk_samples to span the full sample axis");
    }
    if (blosc1_set_compressor(compressor_name_) < 0) {
      throw std::runtime_error("blosc1_set_compressor failed");
    }

    const int tile_grid_ilines = (ilines + chunk_ilines_ - 1) / chunk_ilines_;
    const int tile_grid_xlines = (xlines + chunk_xlines_ - 1) / chunk_xlines_;
    std::vector<std::uint8_t> output;
    output.reserve(static_cast<std::size_t>(ilines) * static_cast<std::size_t>(xlines) * sizeof(float) / 2);

    for (int tile_i = 0; tile_i < tile_grid_ilines; ++tile_i) {
      for (int tile_x = 0; tile_x < tile_grid_xlines; ++tile_x) {
        const auto tile = gather_padded_tile(input, ilines, xlines, samples, tile_i, tile_x, chunk_ilines_, chunk_xlines_);
        const auto input_bytes = static_cast<int32_t>(tile.size() * sizeof(float));
        std::vector<std::uint8_t> compressed(static_cast<std::size_t>(input_bytes) + BLOSC2_MAX_OVERHEAD);
        const int compressed_size =
            blosc1_compress(compression_level, BLOSC_BITSHUFFLE, static_cast<int32_t>(sizeof(float)), input_bytes,
                            tile.data(), compressed.data(), static_cast<int32_t>(compressed.size()));
        if (compressed_size <= 0) {
          throw std::runtime_error("blosc1_compress failed");
        }
        append_u64_le(output, static_cast<std::uint64_t>(compressed_size));
        output.insert(output.end(), compressed.begin(), compressed.begin() + compressed_size);
      }
    }
    return output;
  }

  std::vector<float> decompress(const std::vector<std::uint8_t> &input,
                                int ilines,
                                int xlines,
                                int samples) const override {
    if (chunk_samples_ != samples) {
      throw std::runtime_error("TileBloscCodec expects chunk_samples to span the full sample axis");
    }
    const int tile_grid_ilines = (ilines + chunk_ilines_ - 1) / chunk_ilines_;
    const int tile_grid_xlines = (xlines + chunk_xlines_ - 1) / chunk_xlines_;
    const std::size_t chunk_value_count =
        static_cast<std::size_t>(chunk_ilines_) * static_cast<std::size_t>(chunk_xlines_) * static_cast<std::size_t>(samples);
    std::vector<float> output(static_cast<std::size_t>(ilines) * static_cast<std::size_t>(xlines) *
                              static_cast<std::size_t>(samples), 0.0f);
    std::vector<float> tile(chunk_value_count);
    std::size_t offset = 0;
    for (int tile_i = 0; tile_i < tile_grid_ilines; ++tile_i) {
      for (int tile_x = 0; tile_x < tile_grid_xlines; ++tile_x) {
        const std::uint64_t compressed_size = read_u64_le(input, offset);
        if (offset + compressed_size > input.size()) {
          throw std::runtime_error("Corrupt chunked stream payload");
        }
        const int decompressed_size = blosc1_decompress(input.data() + offset, tile.data(),
                                                        static_cast<int32_t>(tile.size() * sizeof(float)));
        if (decompressed_size < 0) {
          throw std::runtime_error("blosc1_decompress failed");
        }
        offset += static_cast<std::size_t>(compressed_size);
        scatter_padded_tile(tile, ilines, xlines, samples, tile_i, tile_x, chunk_ilines_, chunk_xlines_, output);
      }
    }
    if (offset != input.size()) {
      throw std::runtime_error("Chunked stream trailing bytes detected");
    }
    return output;
  }

private:
  const char *compressor_name_;
  int chunk_ilines_;
  int chunk_xlines_;
  int chunk_samples_;
};

static std::unique_ptr<CodecSpec> parse_codec(std::string_view codec_name,
                                              int compression_level,
                                              int tile_ilines,
                                              int tile_xlines,
                                              int tile_samples) {
  if (codec_name == "fpzip") {
    return std::make_unique<FpzipCodec>(compression_level);
  }
  if (codec_name == "blosc2-zstd-bitshuffle") {
    return std::make_unique<BloscCodec>("blosc2-zstd-bitshuffle", BLOSC_ZSTD_COMPNAME, compression_level);
  }
  if (codec_name == "blosc2-lz4-bitshuffle") {
    return std::make_unique<BloscCodec>("blosc2-lz4-bitshuffle", BLOSC_LZ4_COMPNAME, compression_level);
  }
  if (codec_name == "trace-xor-blosc2-zstd-bitshuffle") {
    return std::make_unique<TraceXorBloscCodec>("trace-xor-blosc2-zstd-bitshuffle", BLOSC_ZSTD_COMPNAME,
                                                compression_level);
  }
  if (codec_name == "trace-xor-blosc2-lz4-bitshuffle") {
    return std::make_unique<TraceXorBloscCodec>("trace-xor-blosc2-lz4-bitshuffle", BLOSC_LZ4_COMPNAME,
                                                compression_level);
  }
  if (codec_name == "tile-blosc2-zstd-bitshuffle") {
    return std::make_unique<TileBloscCodec>("tile-blosc2-zstd-bitshuffle", BLOSC_ZSTD_COMPNAME, compression_level,
                                            tile_ilines, tile_xlines, tile_samples);
  }
  if (codec_name == "tile-blosc2-lz4-bitshuffle") {
    return std::make_unique<TileBloscCodec>("tile-blosc2-lz4-bitshuffle", BLOSC_LZ4_COMPNAME, compression_level,
                                            tile_ilines, tile_xlines, tile_samples);
  }
  throw std::runtime_error(
      "Unsupported codec. Expected one of: fpzip, blosc2-zstd-bitshuffle, blosc2-lz4-bitshuffle, "
      "trace-xor-blosc2-zstd-bitshuffle, trace-xor-blosc2-lz4-bitshuffle, "
      "tile-blosc2-zstd-bitshuffle, tile-blosc2-lz4-bitshuffle");
}

static BenchResult benchmark_codec(const std::string &dataset_name,
                                   int ilines,
                                   int xlines,
                                   int samples,
                                   const std::vector<float> &source,
                                   const CodecSpec &codec,
                                   const fs::path &output_dir) {
  fs::create_directories(output_dir);
  const auto input_bytes = static_cast<std::uintmax_t>(source.size() * sizeof(float));

  const auto compression_started = std::chrono::steady_clock::now();
  const auto compressed = codec.compress(source, ilines, xlines, samples);
  const auto compression_ms = elapsed_ms(compression_started);

  const fs::path compressed_path = output_dir / (dataset_name + "." + codec.label + ".bin");
  write_bytes(compressed_path, compressed);

  const auto decompression_started = std::chrono::steady_clock::now();
  const auto decompressed = codec.decompress(compressed, ilines, xlines, samples);
  const auto decompression_ms = elapsed_ms(decompression_started);

  const bool exact_roundtrip =
      decompressed.size() == source.size() &&
      std::memcmp(decompressed.data(), source.data(), source.size() * sizeof(float)) == 0;

  const int mid_inline = ilines / 2;
  const int mid_xline = xlines / 2;

  const auto inline_started = std::chrono::steady_clock::now();
  const auto inline_volume = codec.decompress(compressed, ilines, xlines, samples);
  const auto inline_section = extract_inline_section(inline_volume, ilines, xlines, samples, mid_inline);
  (void)inline_section;
  const auto decode_inline_section_ms = elapsed_ms(inline_started);

  const auto xline_started = std::chrono::steady_clock::now();
  const auto xline_volume = codec.decompress(compressed, ilines, xlines, samples);
  const auto xline_section = extract_xline_section(xline_volume, ilines, xlines, samples, mid_xline);
  (void)xline_section;
  const auto decode_xline_section_ms = elapsed_ms(xline_started);

  const auto preview_started = std::chrono::steady_clock::now();
  auto preview_volume = codec.decompress(compressed, ilines, xlines, samples);
  auto preview_inline = extract_inline_section(preview_volume, ilines, xlines, samples, mid_inline);
  apply_pipeline_to_traces(preview_inline, xlines, samples, 2.0f);
  const auto decode_preview_pipeline_ms = elapsed_ms(preview_started);

  const auto apply_started = std::chrono::steady_clock::now();
  auto apply_volume = codec.decompress(compressed, ilines, xlines, samples);
  apply_pipeline_to_traces(apply_volume, ilines * xlines, samples, 2.0f);
  const auto decode_apply_pipeline_ms = elapsed_ms(apply_started);

  return BenchResult{
      dataset_name,
      codec.label,
      ilines,
      xlines,
      samples,
      codec.compression_level,
      input_bytes,
      compressed.size(),
      compressed.empty() ? 0.0 : static_cast<double>(input_bytes) / static_cast<double>(compressed.size()),
      compression_ms,
      decompression_ms,
      decode_inline_section_ms,
      decode_xline_section_ms,
      decode_preview_pipeline_ms,
      decode_apply_pipeline_ms,
      exact_roundtrip,
  };
}

static void print_json(const BenchResult &result) {
  std::cout << std::fixed << std::setprecision(3);
  std::cout << "{\n"
            << "  \"dataset_name\": \"" << result.dataset_name << "\",\n"
            << "  \"codec\": \"" << result.codec << "\",\n"
            << "  \"shape\": [" << result.ilines << ", " << result.xlines << ", " << result.samples << "],\n"
            << "  \"compression_level\": " << result.compression_level << ",\n"
            << "  \"input_store_bytes\": " << result.input_store_bytes << ",\n"
            << "  \"compressed_bytes\": " << result.compressed_bytes << ",\n"
            << "  \"compression_ratio\": " << result.compression_ratio << ",\n"
            << "  \"compression_ms\": " << result.compression_ms << ",\n"
            << "  \"decompression_ms\": " << result.decompression_ms << ",\n"
            << "  \"decode_inline_section_ms\": " << result.decode_inline_section_ms << ",\n"
            << "  \"decode_xline_section_ms\": " << result.decode_xline_section_ms << ",\n"
            << "  \"decode_preview_pipeline_ms\": " << result.decode_preview_pipeline_ms << ",\n"
            << "  \"decode_apply_pipeline_ms\": " << result.decode_apply_pipeline_ms << ",\n"
            << "  \"exact_roundtrip\": " << (result.exact_roundtrip ? "true" : "false") << "\n"
            << "}\n";
}

int main(int argc, char **argv) {
  const bool tbvol_mode = argc >= 2 && std::string_view(argv[1]) == "--tbvol";
  if ((!tbvol_mode && argc != 7 && argc != 8) || (tbvol_mode && argc != 5 && argc != 6 && argc != 8 && argc != 9)) {
    std::cerr << "usage: lossless_float_storage_bench <dataset-name> <ilines> <xlines> <samples> <codec> "
                 "<output-dir> [compression-level]\n";
    std::cerr << "   or: lossless_float_storage_bench --tbvol <tbvol-dir> <codec> <output-dir> [compression-level]\n";
    std::cerr << "   or: lossless_float_storage_bench --tbvol <tbvol-dir> <codec> <output-dir> --tile-shape <ci,cx,cs> [compression-level]\n";
    std::cerr << "codec: fpzip | blosc2-zstd-bitshuffle | blosc2-lz4-bitshuffle | "
                 "trace-xor-blosc2-zstd-bitshuffle | trace-xor-blosc2-lz4-bitshuffle | "
                 "tile-blosc2-zstd-bitshuffle | tile-blosc2-lz4-bitshuffle\n";
    return 1;
  }

  std::string dataset_name;
  int ilines = 0;
  int xlines = 0;
  int samples = 0;
  std::string codec_name;
  fs::path output_dir;
  int compression_level = 5;
  std::vector<float> source;
  int tile_ilines = 0;
  int tile_xlines = 0;
  int tile_samples = 0;

  try {
    if (tbvol_mode) {
      const fs::path tbvol_dir = argv[2];
      codec_name = argv[3];
      output_dir = argv[4];
      int next_arg = 5;
      bool has_tile_override = false;
      std::string tile_override;
      if (argc >= 8) {
        if (std::string_view(argv[5]) != "--tile-shape") {
          throw std::runtime_error("Expected --tile-shape before tile override value");
        }
        has_tile_override = true;
        tile_override = argv[6];
        next_arg = 7;
      }
      compression_level = argc > next_arg ? std::stoi(argv[next_arg]) : 5;

      const auto volume = load_tbvol_volume(tbvol_dir);
      dataset_name = volume.dataset_name;
      ilines = volume.ilines;
      xlines = volume.xlines;
      samples = volume.samples;
      tile_ilines = volume.tile_ilines;
      tile_xlines = volume.tile_xlines;
      tile_samples = volume.tile_samples;
      if (has_tile_override) {
        std::regex tile_pattern(R"(^\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*$)");
        std::smatch tile_match;
        if (!std::regex_match(tile_override, tile_match, tile_pattern)) {
          throw std::runtime_error("Invalid --tile-shape override. Expected ci,cx,cs");
        }
        tile_ilines = std::stoi(tile_match[1].str());
        tile_xlines = std::stoi(tile_match[2].str());
        tile_samples = std::stoi(tile_match[3].str());
      }
      source = volume.values;
    } else {
      dataset_name = argv[1];
      ilines = std::stoi(argv[2]);
      xlines = std::stoi(argv[3]);
      samples = std::stoi(argv[4]);
      codec_name = argv[5];
      output_dir = argv[6];
      compression_level = argc == 8 ? std::stoi(argv[7]) : 5;

      if (ilines <= 0 || xlines <= 0 || samples <= 0) {
        std::cerr << "shape values must be positive\n";
        return 1;
      }
      tile_ilines = ilines;
      tile_xlines = xlines;
      tile_samples = samples;
      source = make_synthetic_volume(ilines, xlines, samples);
    }
  } catch (const std::exception &error) {
    std::cerr << error.what() << "\n";
    return 1;
  }

  if (compression_level < 0 || compression_level > 9) {
    std::cerr << "compression-level must be between 0 and 9\n";
    return 1;
  }

  blosc2_init();
  try {
    auto codec = parse_codec(codec_name, compression_level, tile_ilines, tile_xlines, tile_samples);
    const auto result = benchmark_codec(dataset_name, ilines, xlines, samples, source, *codec, output_dir);
    print_json(result);
    blosc2_destroy();
    return 0;
  } catch (const std::exception &error) {
    std::cerr << error.what() << "\n";
    blosc2_destroy();
    return 2;
  }
}

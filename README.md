# Audio to Tonie Converter

[![Tests](https://img.shields.io/github/actions/workflow/status/hotzenklotz/audio2tonie/ci.yml?branch=main&style=flat&label=Tests
)](https://github.com/hotzenklotz/audio2tonie/actions/workflows/ci.yml)

This project is a command-line interface (CLI) application that converts audio files to Ogg Vorbis format with a custom header for compatibility with the Toniebox file format.

## Installation

### Option 1: Download Pre-built Binary

Download the latest release for your platform from the [GitHub releases page](https://github.com/hotzenklotz/audio2tonie/releases):

- Windows: `audio2tonie-windows.zip`
- Linux: `audio2tonie-linux.zip`
- macOS: `audio2tonie-macos.zip`

Extract the zip file and make the binary executable (on Linux/macOS):
```bash
chmod +x audio2tonie
```

### Option 2: Build from Source

If you prefer to build from source:

```bash
cargo install --path .
```

## Usage

The application provides two main commands:

### 1. Extract Toniefile (TAF) to Opus

Extract the audio content from a Tonie file and save it as a new Ogg Opus file.

```bash
audio2tonie extract <input_file> [output_directory]
```

Example:
```bash
audio2tonie extract my_tonie_file.ogg ./extracted_audio
```

### 2. Convert audio file to Tonie (TAF)

Convert a single audio file or a directory of audio files into a Toniebox compatible audio file. Input audio files can be in any format supported by ffmpeg, e.g. MP3, AAC, WAV, OGG, WEBM, OPUS etc.

```bash
audio2tonie convert <input_path> <output_file> [--ffmpeg <ffmpeg_path>]
```

Parameters:
- `input_path`: Path to the input audio file or directory
- `output_file`: Path for the output file (default: "500304E0")
- `--ffmpeg`: Path to ffmpeg executable (default: "ffmpeg")

Examples:
```bash
# Convert a single file
audio2tonie convert my_audio.mp3 output.ogg

# Convert all files in a directory
audio2tonie convert ./my_audio_files/ output.ogg

# Specify custom ffmpeg path
audio2tonie convert input.mp3 output.ogg --ffmpeg /usr/local/bin/ffmpeg
```

## Running Tests

To run the test suite:

```bash
cargo test
```

## Requirements

- `ffmpeg` (must be installed and available in PATH or specified via --ffmpeg parameter)
- opus audio codec / libopus ([Installation Hints](https://github.com/shardlab/discordrb/wiki/Installing-libopus))
- Rust (latest stable version for building from source)

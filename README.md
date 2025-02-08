# mp3-to-tonie/mp3-to-tonie/README.md

# MP3 to Tonie Converter

This project is a command-line interface (CLI) application that converts MP3 files to Ogg Vorbis format with a custom header for compatibility with the Toniebox file format.

## Features

- Convert a single MP3 file or an entire directory of MP3 files.
- Generate Ogg Vorbis files with a custom Tonie header.
- Simple command-line interface for ease of use.

## Installation

To build and run the project, ensure you have Rust and Cargo installed. You can install Rust from [rust-lang.org](https://www.rust-lang.org/).

Clone the repository:

```
git clone https://github.com/yourusername/mp3-to-tonie.git
cd mp3-to-tonie
```

Build the project:

```
cargo build --release
```

## Usage

To convert MP3 files, use the following command:

```
./target/release/mp3-to-tonie <input_file_or_directory> <output_file>
```

- `<input_file_or_directory>`: The path to a single MP3 file or a directory containing MP3 files.
- `<output_file>`: The desired output file name for the converted Ogg Vorbis file.

### Example

Convert a single MP3 file:

```
./target/release/mp3-to-tonie song.mp3 output.ogg
```

Convert all MP3 files in a directory:

```
./target/release/mp3-to-tonie /path/to/mp3/files /path/to/output/directory
```

## License

This project is licensed under the MIT License. See the LICENSE file for more details.
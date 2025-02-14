use crate::Converter;
use assert_fs::prelude::*;
use std::io::Read;
use std::path::Path;
use std::fs::File;

const TEST_FILES_DIR: &str = "src/tests/test_files";
const TIMESTAMP: u32 = 1739039539;

#[test]
fn test_create_tonie_from_single_file() -> anyhow::Result<()> {
    let temp = assert_fs::TempDir::new()?;
    let test_mp3_file = Path::new(TEST_FILES_DIR).join("test_1.mp3");
    let expected_taf_file = Path::new(TEST_FILES_DIR).join("test_1.1739039539.taf");

    let converter = Converter::new();
    let output_file = temp.child("output.taf");
    let input_files = vec![test_mp3_file];

    converter.create_tonie_file(
        output_file.to_path_buf(),
        input_files,
        false,
        Some(TIMESTAMP.to_string()),
        96,
        false,
        "ffmpeg",
        "opusenc",
    )?;

    output_file.assert(predicates::path::exists());
    
    // Add file comparison check here
    let mut expected_file = File::open(expected_taf_file)?;
    let mut output_file_handle = File::open(output_file.path())?;

    let mut expected_bytes = Vec::new();
    expected_file.read_to_end(&mut expected_bytes)?;

    let mut output_bytes = Vec::new();
    output_file_handle.read_to_end(&mut output_bytes)?;

    assert_eq!(expected_bytes, output_bytes, "File content differs");

    Ok(())
}

#[test]
fn test_create_tonie_from_multiple_files() -> anyhow::Result<()> {
    let temp = assert_fs::TempDir::new()?;
    let test_mp3_file_1 = Path::new(TEST_FILES_DIR).join("test_1.mp3");
    let test_mp3_file_2 = Path::new(TEST_FILES_DIR).join("test_2.mp3");
    let expected_taf_file = Path::new(TEST_FILES_DIR).join("test_2.1739039539.taf");

    let converter = Converter::new();
    let output_file = temp.child("output.taf");
    let input_files = vec![test_mp3_file_1, test_mp3_file_2];

    converter.create_tonie_file(
        output_file.to_path_buf(),
        input_files,
        false,
        Some(TIMESTAMP.to_string()),
        96,
        false,
        "ffmpeg",
        "opusenc",
    )?;

    output_file.assert(predicates::path::exists());
    // Add file comparison check here
    let mut expected_file = File::open(expected_taf_file)?;
    let mut output_file_handle = File::open(output_file.path())?;

    let mut expected_bytes = Vec::new();
    expected_file.read_to_end(&mut expected_bytes)?;

    let mut output_bytes = Vec::new();
    output_file_handle.read_to_end(&mut output_bytes)?;

    assert_eq!(expected_bytes, output_bytes, "File content differs");

    Ok(())
}

#[test]
fn test_get_opus_tempfile() -> anyhow::Result<()> {
    let test_mp3_file = Path::new(TEST_FILES_DIR).join("test_1.mp3");

    let converter = Converter::new();
    let mut temp_opus_file = converter
        .get_opus_tempfile("ffmpeg", "opusenc", &test_mp3_file, 96, true)?;

    // Check that the file is an Ogg/Opus file (very basic check)
    let mut buffer = [0; 4];
    temp_opus_file.read_exact(&mut buffer)?;

    assert_eq!(&buffer, b"OggS");
    Ok(())
}

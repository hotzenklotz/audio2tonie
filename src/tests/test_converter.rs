use assert_fs::prelude::*;
use crate::Converter;
use std::path::Path;

const TEST_FILES_DIR: &str = "tests/test_files";
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
        output_file.path(),
        &input_files,
        false,
        Some(TIMESTAMP),
        96,
        false,
        "ffmpeg",
        "opusenc",
    )?;

    output_file.assert(predicates::path::exists());
    // Add file comparison check here

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
        output_file.path(),
        &input_files,
        false,
        Some(TIMESTAMP),
        96,
        false,
        "ffmpeg",
        "opusenc",
    )?;

    output_file.assert(predicates::path::exists());
    // Add file comparison check here

    Ok(())
}
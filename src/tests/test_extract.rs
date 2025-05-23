use std::{
    fs::File,
    io::Read,
    os::unix::fs::MetadataExt,
    path::Path,
    ffi::OsStr
};

use anyhow::{Ok, Result};
use glob::glob;
use tempfile::{Builder, tempdir};

use crate::extract::extract_tonie_to_opus;

const TEST_FILES_DIR: &str = env!("CARGO_MANIFEST_DIR");
const TEST_TONIE_FILE: &str = "resources/test/test_1.taf";
const TEST_TONIE_FILE_WITH_CHAPTERS: &str = "resources/test/multiple_chapters.taf";

#[test]
fn test_extract_tonie_to_opus_without_output_path() -> Result<()> {
    // Test the "extract" command without any given output path.
    // Expect to reuse the input file name with ".ogg" extension in a temporary directory.

    let test_tonie_path = Path::new(TEST_FILES_DIR).join(TEST_TONIE_FILE);
    let temp_dir = tempdir()?; // Create a temporary directory.
    // When output_file_path is a directory, extract_tonie_to_opus creates "extracted_toniefile.ogg" in it.
    let expected_output_path = temp_dir.path().join("extracted_toniefile.ogg");

    // Pass the temporary directory path to the function.
    extract_tonie_to_opus(&test_tonie_path, Some(temp_dir.path().to_path_buf()))?;

    let mut expected_output_file = File::open(&expected_output_path)?;
    let mut audio_data: Vec<u8> = vec![0; 10];
    expected_output_file.read_exact(&mut audio_data)?;

    assert!(expected_output_path.exists());
    assert!(expected_output_file.metadata()?.size() > 0);
    assert!(audio_data.starts_with(b"OggS"));

    Ok(())
}

#[test]
fn test_extract_tonie_to_opus_with_output_path() -> Result<()> {
    // Test the "extract" command with just an output directory given, but no specify file name.
    // Expect to reuse the input file name with ".ogg" extension in the specified temporary directory.

    let test_tonie_path = Path::new(TEST_FILES_DIR).join(TEST_TONIE_FILE);
    let temp_dir = tempdir()?; // Create a temporary directory.
    // output_path is the temporary directory.
    let output_path = temp_dir.path().to_path_buf(); 
    // When output_file_path is a directory, extract_tonie_to_opus creates "extracted_toniefile.ogg" in it.
    let expected_output_path = output_path.join("extracted_toniefile.ogg");
    
    // Pass the temporary directory path to the function.
    extract_tonie_to_opus(&test_tonie_path, Some(output_path))?;

    let expected_output_file = File::open(&expected_output_path)?;

    assert!(expected_output_path.exists());
    assert!(expected_output_file.metadata()?.size() > 0);

    Ok(())
}

#[test]
fn test_extract_tonie_to_opus_with_output_file_name() -> Result<()> {
    // Test the "extract" command with just an output path given, including a specified file name.
    // Expect to use the specified output directory and file name.

    let test_tonie_path = Path::new(TEST_FILES_DIR).join(TEST_TONIE_FILE);
    let expected_output_file = Builder::new().suffix(".opus").tempfile()?;

    extract_tonie_to_opus(
        &test_tonie_path,
        Some(expected_output_file.path().to_path_buf()),
    )?;

    assert!(expected_output_file.as_file().metadata()?.size() > 0);

    Ok(())
}

#[test]
fn test_extract_tonie_to_opus_with_multiple_chapters() -> Result<()> {
    // Test the "extract" command with a Tonie file that contains multiple chapters.
    // Expect to extract each chapter into a separate audio file.
    let test_tonie_path = Path::new(TEST_FILES_DIR).join(TEST_TONIE_FILE_WITH_CHAPTERS);
    let expected_output_dir = Builder::new().prefix("tonie_test_dir").tempdir()?;

    extract_tonie_to_opus(
        &test_tonie_path,
        Some(expected_output_dir.path().to_path_buf()),
    )?;

    let glob_path = expected_output_dir.path().join("*.ogg");
    let glob_path_str = glob_path.to_str().unwrap();
    let ogg_files = glob(glob_path_str)?
        .filter_map(Result::ok)
        .collect::<Vec<_>>();

    assert_eq!(ogg_files.len(), 3);

    for ogg_file in ogg_files {
        assert!(ogg_file
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap()
            .starts_with(char::is_numeric));
    }

    Ok(())
}

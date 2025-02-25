use std::{
    fs::File,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

use anyhow::Result;
use tempfile::Builder;
use toniefile::Toniefile;

use crate::extract::extract_tonie_to_opus;

const TEST_FILES_DIR: &str = "src/tests/test_files";
const TEST_TONIE_FILE: &str = "test_1.taf";

#[test]
fn test_extract_tonie_to_opus_without_output_path() -> Result<()> {
    // Test the "extract" command without any given output path.
    // Expect to reuse the input file name with ".ogg" extension in the current working directory.
    
    let test_tonie_path = Path::new(TEST_FILES_DIR).join(TEST_TONIE_FILE);
    let expected_output_path =
    PathBuf::from(".").join(test_tonie_path.with_extension("ogg").file_name().unwrap());
    let expected_output_file = File::open(&expected_output_path)?;
    
    extract_tonie_to_opus(&test_tonie_path, None)?;
    
    assert!(expected_output_path.exists());
    assert!(expected_output_file.metadata()?.size() > 0);

    
    Ok(())
}

#[test]
fn test_extract_tonie_to_opus_with_output_path() -> Result<()> {
    // Test "extract" command with just an output directory given, but no specify file name.
    // Expect to reuse the input file name with ".ogg" extension in the specified directory.
    
    let test_tonie_path = Path::new(TEST_FILES_DIR).join(TEST_TONIE_FILE);
    let output_path = PathBuf::from(".");
    let expected_output_path =
    output_path.join(test_tonie_path.with_extension("ogg").file_name().unwrap());
    let expected_output_file = File::open(&expected_output_path)?;
    
    extract_tonie_to_opus(&test_tonie_path, Some(output_path.clone()))?;
    
    assert!(expected_output_path.exists());
    assert!(expected_output_file.metadata()?.size() > 0);
    
    Ok(())
}

#[test]
fn test_extract_tonie_to_opus_with_output_file_name() -> Result<()> {
    // Test "extract" command with just an output path given, including a specified file name.
    // Expect to use the specified output directory and file name.

    let test_tonie_path = Path::new(TEST_FILES_DIR).join(TEST_TONIE_FILE);
    let expected_output_file = Builder::new().suffix(".opus").tempfile()?;

    extract_tonie_to_opus(&test_tonie_path, Some(expected_output_file.path().to_path_buf()))?;

    assert!(expected_output_file.as_file().metadata()?.size() > 0);

    Ok(())
}

#[test]
fn test_extract_audio_from_toniefile() -> anyhow::Result<()> {
    let test_taf_file_path = Path::new(TEST_FILES_DIR).join("test_1.1739039539.taf");
    let mut test_taf_file = File::open(test_taf_file_path)?;

    let audio_data = Toniefile::extract_audio(&mut test_taf_file)?;

    assert!(audio_data.starts_with(b"OggS"));
    assert!(audio_data.len() > 0);

    Ok(())
}

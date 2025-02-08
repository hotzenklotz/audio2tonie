use assert_fs::prelude::*;
use crate::utils::split_to_opus_files;
use std::path::Path;

const TEST_FILES_DIR: &str = "tests/test_files";

#[test]
fn test_split_tonie_to_single_ogg_file() -> anyhow::Result<()> {
    let temp = assert_fs::TempDir::new()?;
    let test_mp3_file = Path::new(TEST_FILES_DIR).join("test_1.1739039539.taf");

    let output_files = split_to_opus_files(&test_mp3_file, Some(temp.path()))?;

    assert_eq!(output_files.len(), 1);
    assert!(output_files[0].exists());
    assert_eq!(
        output_files[0].file_name().unwrap(),
        "01_test_1.1739039539.opus"
    );

    Ok(())
}
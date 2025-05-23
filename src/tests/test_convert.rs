use anyhow::Result;
use rand::rng;
use rand::seq::SliceRandom;
use std::{
    fs::File, 
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};
use tempfile::{tempdir, NamedTempFile};
use toniefile::Toniefile;

use crate::{
    convert::{audiofile_to_wav, convert_to_tonie, filter_input_files},
    utils::are_files_equal,
};

const TEST_FILES_DIR: &str = env!("CARGO_MANIFEST_DIR");
const TEST_TONIE_FILE: &str = "resources/test/test_1.taf";
const TEST_MP3_FILE: &str = "resources/test/test_1.mp3";

#[test]
fn test_convert_to_tonie_from_single_file() -> anyhow::Result<()> {
    let test_mp3_path = std::fs::canonicalize(Path::new(TEST_FILES_DIR).join(TEST_MP3_FILE))
        .expect("Failed to canonicalize test MP3 path");
    let test_tonie_file_path = std::fs::canonicalize(Path::new(TEST_FILES_DIR).join(TEST_TONIE_FILE))
        .expect("Failed to canonicalize test Tonie file path");
    let test_tonie_file = File::open(test_tonie_file_path)?;
    let temp_file = NamedTempFile::new()?;

    let converted_file = convert_to_tonie(
        &test_mp3_path,
        &temp_file.path().to_path_buf(),
        String::from("ffmpeg"),
    )?;

    assert!(converted_file.metadata()?.size() > 0);
    assert!(are_files_equal(test_tonie_file, temp_file.into_file())?);

    Ok(())
}

#[test]
fn test_convert_to_tonie_from_directory() -> anyhow::Result<()> {
    let temp_dir = tempdir()?.into_path();
    let test_input_path = std::fs::canonicalize(Path::new(TEST_FILES_DIR).join("resources").join("test"))
        .expect("Failed to canonicalize test input directory path");
    let temp_output_path = temp_dir.join("test_tonie.taf");

    let converted_file = convert_to_tonie(&test_input_path, &temp_output_path, String::from("ffmpeg"))?;

    assert!(converted_file.metadata()?.size() > 0);

    let mut temp_output_file = File::open(temp_output_path)?;
    let header = Toniefile::parse_header(&mut temp_output_file)?;

    assert_eq!(header.track_page_nums.len(), 3);

    Ok(())
}

#[test]
fn test_convert_to_tonie_with_default_output() -> anyhow::Result<()> {
    let test_input_path = std::fs::canonicalize(PathBuf::from(TEST_FILES_DIR))
        .expect("Failed to canonicalize test input directory path");
    let temp_output_path = tempdir()?.into_path();

    let converted_file =
        convert_to_tonie(&test_input_path, &temp_output_path, String::from("ffmpeg"))?;

    assert!(converted_file.metadata()?.size() > 0);

    Ok(())
}

#[test]
fn test_convert_to_tonie_with_two_directories() -> anyhow::Result<()> {
    let test_input_path = std::fs::canonicalize(Path::new(TEST_FILES_DIR).join("resources").join("test"))
        .expect("Failed to canonicalize test input directory path");
    let temp_output_path = tempdir()?.into_path();
    let expected_output_path = temp_output_path.join("500304E0");

    let converted_file =
        convert_to_tonie(&test_input_path, &temp_output_path, String::from("ffmpeg"))?;

    assert!(converted_file.metadata()?.size() > 0);
    assert!(expected_output_path.exists());

    Ok(())
}

#[test]
fn test_audiofile_to_wav() -> Result<()> {
    let test_mp3_path = std::fs::canonicalize(Path::new(TEST_FILES_DIR).join(TEST_MP3_FILE))
        .expect("Failed to canonicalize test MP3 path");
    // Revert to using "ffmpeg" assuming it's in PATH
    let temp_wav_buffer = audiofile_to_wav(&test_mp3_path, "ffmpeg")?;

    assert_eq!(temp_wav_buffer.len() / (2 * 2 * 48000), 208); // Stereo = 2 channel á 48000Hz; 2 bytes per second

    Ok(())
}

#[test]
fn test_filter_input_files() -> Result<()> {
    let temp_dir = tempdir()?;
    let temp_path = std::fs::canonicalize(temp_dir.path())
        .expect("Failed to canonicalize temp directory path");

    let mut temp_input_files = vec![
        temp_path.join("1. MyFile.mp3"),
        temp_path.join("2. MyFile.mp3"),
        temp_path.join("3. MyFile.mp3"),
        temp_path.join("10. MyFile.mp3"),
        temp_path.join("20. MyFile.mp3"),
        temp_path.join("MyFile 1.mp3"),
        temp_path.join("MyFile 2.mp3"),
        temp_path.join("MyFile.mp3"),
        temp_path.join("MyFile_1.mp3"),
        temp_path.join("MyFile_2.mp3"),
    ];
    for file_name in &temp_input_files {
        File::create(file_name)?;
    }

    // filter_input_files expects a directory, so we use temp_path which is already canonicalized.
    let validated_paths = filter_input_files(&temp_path)?; 
    assert_eq!(temp_input_files, validated_paths);

    // Shuffle file name order. This should conflict with the sorted and validated input files
    temp_input_files.shuffle(&mut rng());
    assert_ne!(temp_input_files, validated_paths);

    Ok(())
}

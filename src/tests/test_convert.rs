use anyhow::Result;
use std::{
    fs::{self, File},
    os::unix::fs::MetadataExt,
    path::Path,
};
use toniefile::Toniefile;

use crate::convert::{audiofile_to_wav, convert_to_tonie};

const TEST_FILES_DIR: &str = "src/tests/test_files";
const TEST_TONIE_FILE: &str = "test_1.1739039539.taf";

#[test]
fn test_convert_to_tonie_from_single_file() -> anyhow::Result<()> {
    let test_mp3_path = Path::new(TEST_FILES_DIR).join("test_1.mp3");

    let temp_dir = assert_fs::TempDir::new()?;
    let temp_output_path = temp_dir.join("test_tonie.taf");

    let converted_file =
        convert_to_tonie(&test_mp3_path, &temp_output_path, String::from("ffmpeg"))?;
    assert!(converted_file.metadata()?.size() > 0);

    Ok(())
}

#[test]
fn test_convert_to_tonie_from_directory() -> anyhow::Result<()> {
    let temp_dir = assert_fs::TempDir::new()?;
    let test_input_mp3_path = Path::new(TEST_FILES_DIR).join("test_1.mp3");
    let temp_output_path = temp_dir.join("test_tonie.taf");

    // Reuse the same file three times and simulate a directory full individual audio files
    for i in 0..3 {
        fs::copy(
            &test_input_mp3_path,
            temp_dir.join(format!("input_{}.mp3", i)),
        )?;
    }

    let converted_file = convert_to_tonie(
        &temp_dir.to_path_buf(),
        &temp_output_path,
        String::from("ffmpeg"),
    )?;

    assert!(converted_file.metadata()?.size() > 0);

    let mut temp_output_file = File::open(temp_output_path)?;
    let header = Toniefile::parse_header(&mut temp_output_file)?;

    assert_eq!(header.track_page_nums.len(), 3);

    Ok(())
}

#[test]
fn test_audiofile_to_wav() -> Result<()> {
    let test_mp3_path = Path::new(TEST_FILES_DIR).join("test_1.mp3");
    let temp_wav_buffer = audiofile_to_wav(&test_mp3_path, "ffmpeg")?;

    assert_eq!(temp_wav_buffer.len() / (2 * 2 * 48000), 207); // Stereo = 2 channel á 48000Hz; 2 bytes per second

    Ok(())
}

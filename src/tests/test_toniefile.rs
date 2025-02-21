use std::{
    fs::File,
    os::unix::fs::MetadataExt,
    path::Path,
};
use toniefile::Toniefile;

use crate::{convert::audiofile_to_wav, utils::vec_u8_to_i16};

const TEST_FILES_DIR: &str = "src/tests/test_files";
const TEST_TONIE_FILE: &str = "test_1.1739039539.taf";

#[test]
fn test_toniefile_from_file() -> anyhow::Result<()> {
    let test_mp3_file = Path::new(TEST_FILES_DIR).join("test_1.mp3");

    let temp_wav_file = audiofile_to_wav(&test_mp3_file, "ffmpeg")?;
    let temp_wav_file_i16: Vec<i16> = vec_u8_to_i16(temp_wav_file)?;

    assert_eq!(temp_wav_file_i16.len() / (2 * 48000), 207); // Stereo = 2 channel á 48000Hz

    let temp_dir = assert_fs::TempDir::new()?;
    let output_file = File::create(temp_dir.join("test_tonie.taf")).unwrap();

    let mut toniefile = Toniefile::new(&output_file, 0x12345678, None).unwrap();
    toniefile.encode(&temp_wav_file_i16)?;
    toniefile.finalize_no_consume()?;

    assert!(output_file.metadata()?.size() > 0);

    Ok(())
}

#[test]
fn test_parse_header_from_toniefile() -> anyhow::Result<()> {
    let test_taf_file_path = Path::new(TEST_FILES_DIR).join("test_1.1739039539.taf");
    let mut test_taf_file = File::open(test_taf_file_path)?;

    let header = Toniefile::parse_header(&mut test_taf_file)?;

    assert_eq!(
        header.sha1_hash,
        [
            168, 186, 111, 245, 189, 167, 148, 173, 29, 88, 27, 4, 89, 117, 44, 74, 242, 175, 121,
            73
        ]
    );
    assert_eq!(header.num_bytes, 2498560);
    assert_eq!(header.audio_id, 1739039539);
    assert_eq!(header.track_page_nums, [0]);
    assert_eq!(header.fill.len(), 4053);

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

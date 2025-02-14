use sha1::Digest;
use crate::utils::{split_to_opus_files, check_tonie_file, get_header_info, get_audio_info, crc32};
use std::{fs::File, path::Path};

const TEST_FILES_DIR: &str = "src/tests/test_files";
const TEST_TONIE_FILE: &str = "test_1.1739039539.taf";

#[test]
fn test_split_tonie_to_single_ogg_file() -> anyhow::Result<()> {
    let temp = assert_fs::TempDir::new()?;
    let test_tonie_file = Path::new(TEST_FILES_DIR).join("test_1.1739039539.taf");
    
    let output_files = split_to_opus_files(&test_tonie_file, Some(temp.path()))?;
    
    assert_eq!(output_files.len(), 1);
    assert!(output_files[0].exists());
    assert_eq!(
        output_files[0].file_name().unwrap(),
        "01_test_1.1739039539.opus"
    );
    
    Ok(())
}

#[test]
fn test_split_tonie_to_multiple_ogg_files() -> anyhow::Result<()> {
    let temp = assert_fs::TempDir::new()?;
    let test_tonie_file = Path::new(TEST_FILES_DIR).join("test_2.1739039539.taf");
    
    let output_files = split_to_opus_files(&test_tonie_file, Some(temp.path()))?;
    
    assert_eq!(output_files.len(), 2);
    for output_file in output_files.iter(){
        assert!(output_file.exists());
    }

    assert_eq!(
        output_files[0].file_name().unwrap(),
        "01_test_2.1739039539.opus"
    );
    assert_eq!(
        output_files[1].file_name().unwrap(),
        "02_test_2.1739039539.opus"
    );
    
    Ok(())
}

#[test]
fn test_tonie_file() -> anyhow::Result<()> {
    let test_tonie_file = Path::new(TEST_FILES_DIR).join("test_1.1739039539.taf");
    
    let result = check_tonie_file(&test_tonie_file.as_path())?;
    
    assert!(result == true);
    
    Ok(())
}

#[test]
fn test_get_header_info() -> anyhow::Result<()> {
    let test_tonie_file = Path::new(TEST_FILES_DIR).join("test_1.1739039539.taf");
    let mut input_file= File::open(test_tonie_file.as_path())?;

    let (
        header_size,
        tonie_header,
        file_size,
        audio_size,
        sha1sum,
        opus_head_found,
        opus_version,
        channel_count,
        sample_rate,
        bitstream_serial_no,
    ) = get_header_info(&mut input_file)?;

    assert_eq!(
        tonie_header.dataHash
        , [168, 186, 111, 245, 189, 167, 148, 173, 29, 88, 27, 4, 89, 117, 44, 74, 242, 175, 121, 73]
    );
    assert_eq!(tonie_header.dataLength , 2498560);
    assert_eq!(tonie_header.timestamp , 1739039539);
    assert_eq!(tonie_header.chapterPages , [0]);
    assert!(tonie_header.padding.iter().all(|&x| x == 0));
    assert_eq!(tonie_header.padding.len(), 4053);
    
       
    assert_eq!(header_size , 4092);
    assert_eq!(file_size , 2502656);
    assert_eq!(audio_size , 2498560);
    assert_eq!(
        sha1sum.finalize().to_vec() , vec![0xa8, 0xba, 0x6f, 0xf5, 0xbd, 0xa7, 0x94, 0xad, 0x1d, 0x58, 0x1b, 0x04, 0x59, 0x75, 0x2c, 0x4a, 0xf2, 0xaf, 0x79, 0x49]
    );
    assert!(opus_head_found);
    assert_eq!(opus_version , 1);
    assert_eq!(channel_count , 2);
    assert_eq!(sample_rate , 48000);
    assert_eq!(bitstream_serial_no , 1739039539);

    Ok(())
}

#[test]
fn test_get_audio_info() -> anyhow::Result<()> {
    let test_tonie_file = Path::new(TEST_FILES_DIR).join(TEST_TONIE_FILE);
    let mut input_file = File::open(test_tonie_file)?;

    let (
        header_size,
        tonie_header,
        _file_size,
        _audio_size,
        _sha1,
        _opus_head_found,
        _opus_version,
        _channel_count,
        sample_rate,
        _bitstream_serial_no,
    ) = get_header_info(&mut input_file)?;

    let (page_count, alignment_okay, page_size_okay, total_time, chapters) =
        get_audio_info(&mut input_file, sample_rate, &tonie_header, header_size)?;

    assert_eq!(page_count, 612);
    assert!(alignment_okay);
    assert!(page_size_okay);
    assert_eq!(total_time, 207);
    assert_eq!(chapters, ["00:03:27.00"]);

    Ok(())
}

#[test]
fn test_crc32() {
    let test_data: [u8; 20] = [
        0xba, 0xca, 0x6f, 0xf5, 0xbb, 0xa7, 0x94, 0xad, 0x1d, 0x58, 0x1b, 0x04, 0x59, 0x75, 0x2c,
        0x4a, 0xf2, 0xaf, 0x79, 0x49,
    ];

    let crc_checksum = crc32(&test_data);

    assert_eq!(crc_checksum, 4269137275);
}

use crate::opus_packet::OpusPacket;
use crate::ogg_page::{
    OggPage, DO_NOTHING, ONLY_CONVERT_FRAMEPACKING, OTHER_PACKET_NEEDED, TOO_MANY_SEGMENTS,
};
use std::fs::File;
use std::path::Path;

const TEST_FILES_DIR: &str = "src/tests/test_files";
const TEST_TONIE_FILE: &str = "test_1.1739039539.taf";

fn create_ogg_page() -> OggPage {
    let test_tonie_file = Path::new(TEST_FILES_DIR).join(TEST_TONIE_FILE);
    let mut input_file = File::open(test_tonie_file).unwrap();

    OggPage::seek_to_page_header(&mut input_file).ok();
    let ogg_page = OggPage::from_reader(&mut input_file).unwrap();

    return ogg_page;
}

#[test]
fn test_parse_header() {
    let ogg_page = create_ogg_page();

    assert_eq!(ogg_page.version, 0);
    assert_eq!(ogg_page.page_type, 2);
    assert_eq!(ogg_page.granule_position, 0);
    assert_eq!(ogg_page.serial_no, 1739039539);
    assert_eq!(ogg_page.page_no, 0);
    assert_eq!(ogg_page.checksum, 706853594);
    assert_eq!(ogg_page.segment_count, 1);
    assert_eq!(ogg_page.segments.len(), 1);
}

#[test]
fn test_checksum() {
    let ogg_page = create_ogg_page();

    let checksum = ogg_page.calc_checksum();
    assert_eq!(checksum, 706853594);
}

#[test]
fn test_get_page_size() {
    let ogg_page = create_ogg_page();

    let page_size = ogg_page.get_page_size();
    assert_eq!(page_size, 47);
}

#[test]
fn test_get_packet_size() {
    let ogg_page = create_ogg_page();

    let packet_size = ogg_page.get_opus_packet_size(0);
    assert_eq!(packet_size, 19);
}

#[test]
fn test_get_segment_count_of_packet_at() {
    let ogg_page = create_ogg_page();

    let segment_count = ogg_page.get_segment_count_of_packet_at(0);
    assert_eq!(segment_count, 1);
}

// Helper function to create a basic OggPage for testing
fn create_padding_test_page() -> OggPage {
    let mut page = OggPage::new();
    page.version = 0;
    page.page_type = 2;
    page.granule_position = 0;
    page.serial_no = 12345;
    page.page_no = 2;
    page.checksum = 0;

    // Add a single segment OpusPacket
    let mut packet = OpusPacket::new::<std::io::Empty>(None, 10, 0, false).unwrap();
    packet.first_packet = true;
    packet.size = 10;
    packet.data = b"\x00123456789".to_vec(); // Valid TOC byte (0x00)
    packet.frame_count = Some(1);

    page.segments.push(packet);
    page.segment_count = page.segments.len() as u8;

    return page;
}

#[test]
fn test_calc_actual_padding_value_do_nothing() {
    let padding_test_page = create_padding_test_page();
    let bytes_needed = 0;
    let result = padding_test_page
        .calc_actual_padding_value(0, bytes_needed)
        .unwrap();

    assert_eq!(result, DO_NOTHING);
}

#[test]
fn test_calc_actual_padding_value_other_packet_needed() {
    let mut padding_test_page = create_padding_test_page();
    padding_test_page.segments[0].size = 245;
    let bytes_needed = 10;
    let result = padding_test_page
        .calc_actual_padding_value(0, bytes_needed)
        .unwrap();

    assert_eq!(result, OTHER_PACKET_NEEDED);
}

#[test]
fn test_calc_actual_padding_value_only_convert_framepacking() {
    let mut padding_test_page = create_padding_test_page();
    padding_test_page.segments[0].framepacking = -1;
    let bytes_needed = 1;
    let result = padding_test_page
        .calc_actual_padding_value(0, bytes_needed)
        .unwrap();

    assert_eq!(result, ONLY_CONVERT_FRAMEPACKING);
}

#[test]
fn test_calc_actual_padding_value_too_many_segments() {
    let mut padding_test_page = create_padding_test_page();

    let segments = get_segments_by_size(255);
    padding_test_page.segments = segments;

    padding_test_page.segment_count = 255;
    let bytes_needed = 254;
    let result = padding_test_page
        .calc_actual_padding_value(0, bytes_needed)
        .unwrap();

    assert_eq!(result, TOO_MANY_SEGMENTS);
}

#[test]
fn test_calc_actual_padding_value_valid_padding() {
    let padding_test_page = create_padding_test_page();
    let bytes_needed = 5;
    let result = padding_test_page
        .calc_actual_padding_value(0, bytes_needed)
        .unwrap();

    assert_eq!(result, 3);
}

#[test]
fn test_pad_do_nothing() {
    let mut padding_test_page = create_padding_test_page();
    let initial_size = padding_test_page.get_page_size();
    padding_test_page.pad(initial_size, None).unwrap();

    assert_eq!(padding_test_page.get_page_size(), initial_size);
}

#[test]
fn test_pad_one_byte() {
    let mut padding_test_page = create_padding_test_page();
    let initial_size = padding_test_page.get_page_size();
    padding_test_page.pad_one_byte().unwrap();

    assert_eq!(padding_test_page.get_page_size(), initial_size + 1);
}

#[test]
fn test_pad_one_byte_multiple_times2() {
    let mut padding_test_page = create_padding_test_page();
    padding_test_page.segments = get_segments_by_size(10);

    let initial_size = padding_test_page.get_page_size();

    for _i in 0..150 {
        padding_test_page.pad_one_byte().unwrap();
    }

    assert_eq!(padding_test_page.get_page_size(), initial_size + 150);
}

#[test]
fn test_pad_one_byte_multiple_times() {
    let mut padding_test_page = create_padding_test_page();
    let initial_size = padding_test_page.get_page_size();

    for _i in 0..255 {
        padding_test_page.pad_one_byte().unwrap();
    }

    assert_eq!(padding_test_page.get_page_size(), initial_size + 255);
}

#[test]
fn test_pad_valid_padding() {
    let mut padding_test_page = create_padding_test_page();
    padding_test_page.segments[0].padding = Some(0);

    let initial_size = padding_test_page.get_page_size();
    let pad_to_size = initial_size + 5;
    padding_test_page.pad(pad_to_size, None).unwrap();

    assert_eq!(padding_test_page.get_page_size(), pad_to_size);
}

fn get_segments_by_size(size: usize) -> Vec<OpusPacket> {
    let padding_test_page = create_padding_test_page();

    let mut segments = Vec::with_capacity(size);
    for i in 0..size {
        let mut new_segment = padding_test_page.segments[0].clone();
        new_segment.first_packet = i == 0;
        segments.push(new_segment);
    }
    return segments;
}

#[test]
fn test_calc_actual_padding_value_edge_case_1() {
    let mut padding_test_page = create_padding_test_page();

    // Test case 1: bytes_needed is just enough to fill the last segment
    let segments = get_segments_by_size(250);
    padding_test_page.segments = segments;
    
    let bytes_needed = 5;
    let result = padding_test_page
        .calc_actual_padding_value(0, bytes_needed)
        .unwrap();
    assert_eq!(result, 3);
}

#[test]
fn test_calc_actual_padding_value_edge_case_2() {
    let mut padding_test_page = create_padding_test_page();
    // Test case 2: bytes_needed requires adding a new segment
    let segments = get_segments_by_size(250);
    padding_test_page.segments = segments;

    let bytes_needed = 10;
    let result = padding_test_page
        .calc_actual_padding_value(0, bytes_needed)
        .unwrap();
    assert_eq!(result, OTHER_PACKET_NEEDED);
}

#[test]
fn test_calc_actual_padding_value_edge_case_3() {
    let mut padding_test_page = create_padding_test_page();
    // Test case 3: bytes_needed is large and requires multiple new segments
    let segments = get_segments_by_size(100);
    padding_test_page.segments = segments;

    let bytes_needed = 400;
    let result = padding_test_page
        .calc_actual_padding_value(0, bytes_needed)
        .unwrap();
    assert_eq!(result, TOO_MANY_SEGMENTS);
}

#[test]
fn test_calc_actual_padding_value_edge_case_4() {
    let mut padding_test_page = create_padding_test_page();
    // Test case 4: bytes_needed is 1 and convert_framepacking_needed is true
    let segments = get_segments_by_size(10);
    padding_test_page.segments = segments;

    padding_test_page.segments[0].framepacking = -1;
    let bytes_needed = 1;
    let result = padding_test_page
        .calc_actual_padding_value(0, bytes_needed)
        .unwrap();
    assert_eq!(result, ONLY_CONVERT_FRAMEPACKING);
}

#[test]
fn test_calc_actual_padding_value_edge_case_5() {
    let mut padding_test_page = create_padding_test_page();
    // Test case 5: bytes_needed is 1 and convert_framepacking_needed is false
    let segments = get_segments_by_size(10);
    padding_test_page.segments = segments;

    padding_test_page.segments[0].framepacking = 3;
    let bytes_needed = 1;
    let result = padding_test_page
        .calc_actual_padding_value(0, bytes_needed)
        .unwrap();
    assert_eq!(result, 0);
}

#[test]
fn test_calc_actual_padding_value_edge_case_6() {
    let mut padding_test_page = create_padding_test_page();
    // Test case 6: bytes_needed requires framepacking conversion and padding
    let segments = get_segments_by_size(250);
    padding_test_page.segments = segments;
    padding_test_page.segments[0].framepacking = -1;
    
    let bytes_needed = 5;
    let result = padding_test_page
        .calc_actual_padding_value(0, bytes_needed)
        .unwrap();
    assert_eq!(result, OTHER_PACKET_NEEDED);
}

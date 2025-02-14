use crate::opus_page::OpusPage;
use crate::tonie_header::tonie_header::TonieHeader;
use anyhow::{anyhow, Result};
use byteorder::{BigEndian, ReadBytesExt};
use chrono::{TimeZone, Utc};
use protobuf::Message;
use sha1::{Digest, Sha1};
use std::env::current_dir;
use std::f64;
use std::fs::{self, File};
use std::io::Read;
use std::io::{Seek, SeekFrom};
use std::path::{Path, PathBuf};

const SUPPORTED_FILE_TYPES: &[&str] = &[".opus", ".ogg", ".mp3", ".wav", ".m4a"];

pub fn check_tonie_file(path: &Path) -> Result<bool> {
    let mut file = File::open(path)?;

    // Get header info
    let (
        header_size,
        tonie_header,
        _file_size,
        audio_size,
        sha1,
        opus_head_found,
        opus_version,
        channel_count,
        sample_rate,
        bitstream_serial_no,
    ) = get_header_info(&mut file)?;

    // Get audio info
    let (page_count, alignment_okay, page_size_okay, total_time, chapters) =
        get_audio_info(&mut file, sample_rate, &tonie_header, header_size)?;

    let hash_ok = tonie_header.dataHash == sha1.finalize().as_slice();
    let timestamp_ok = tonie_header.timestamp == bitstream_serial_no;
    let audio_size_ok = tonie_header.dataLength as u64 == audio_size;
    let opus_ok = opus_head_found
        && opus_version == 1
        && (sample_rate == 48000 || sample_rate == 44100)
        && channel_count == 2;

    let all_ok = hash_ok && timestamp_ok && opus_ok && alignment_okay && page_size_okay;

    // // Print status information
    // println!(
    //     "[{}] SHA1 hash: 0x{}",
    //     if hash_ok { "OK" } else { "NOT OK" },
    //     hex::encode_upper(&tonie_header.dataHash)
    // );

    // if !hash_ok {
    //     println!("            actual: 0x{}", hex::encode_upper(sha1.finalize()));
    // }

    println!(
        "[{}] Timestamp: [0x{:X}] {}",
        if timestamp_ok { "OK" } else { "NOT OK" },
        tonie_header.timestamp,
        format_time(tonie_header.timestamp as u64)
    );

    if !timestamp_ok {
        println!("   bitstream serial: 0x{:X}", bitstream_serial_no);
    }

    println!(
        "[{}] Opus data length: {} bytes (~{:.0} kbps)",
        if audio_size_ok { "OK" } else { "NOT OK" },
        tonie_header.dataLength,
        (audio_size as f64 * 8.0) / 1024.0 / total_time as f64
    );

    if !audio_size_ok {
        println!("     actual: {} bytes", audio_size);
    }

    println!(
        "[{}] Opus header {}OK || {} channels || {:.1} kHz || {} Ogg pages",
        if opus_ok { "OK" } else { "NOT OK" },
        if opus_head_found && opus_version == 1 {
            ""
        } else {
            "NOT "
        },
        channel_count,
        sample_rate as f64 / 1000.0,
        page_count
    );

    println!(
        "[{}] Page alignment {}OK and size {}OK",
        if alignment_okay && page_size_okay {
            "OK"
        } else {
            "NOT OK"
        },
        if alignment_okay { "" } else { "NOT " },
        if page_size_okay { "" } else { "NOT " }
    );

    println!();
    println!(
        "[{}] File is {}valid",
        if all_ok { "OK" } else { "NOT OK" },
        if all_ok { "" } else { "NOT " }
    );

    println!();
    println!(
        "[ii] Total runtime: {}",
        granule_to_time_string(total_time, sample_rate)
    );
    println!("[ii] {} Tracks:", chapters.len());

    for (i, chapter) in chapters.iter().enumerate() {
        println!("  Track {:02}: {}", i + 1, chapter);
    }

    Ok(all_ok)
}

pub fn get_header_info(
    file: &mut File,
) -> Result<(
    u64,         // header_size
    TonieHeader, // tonie_header
    u64,         // file_size
    u64,         // audio_size
    Sha1,        // sha1
    bool,        // opus_head_found
    u8,          // opus_version
    u8,          // channel_count
    u32,         // sample_rate
    u32,         // bitstream_serial_no
)> {
    // Read header size and header data
    let header_size = file.read_u32::<BigEndian>()? as u64;
    let mut header_bytes = vec![0u8; header_size as usize];
    file.read_exact(&mut header_bytes)?;

    let tonie_header = TonieHeader::parse_from_bytes(&header_bytes)?;

    // Calculate SHA1 of remaining data
    let mut sha1 = Sha1::new();
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;
    sha1.update(&buffer);

    // Get file size and audio size
    let file_size = file.stream_position()?;
    file.seek(SeekFrom::Start(4 + header_size))?;
    let audio_size = file_size - file.stream_position()?;

    // Read first Opus page
    let found = OpusPage::seek_to_page_header(file)?;
    if !found {
        anyhow::bail!("First ogg page not found");
    }
    let first_page = OpusPage::from_reader(file)?;

    // Parse Opus header data
    let segment_data = &first_page.segments[0].data;
    if segment_data.len() < 18 {
        anyhow::bail!("Opus header segment too short");
    }

    let opus_head_found = &segment_data[0..8] == b"OpusHead";
    let opus_version = segment_data[8];
    let channel_count = segment_data[9];
    let sample_rate = u32::from_le_bytes(segment_data[12..16].try_into()?);
    let bitstream_serial_no = first_page.serial_no;

    // Read second page (discard)
    let found = OpusPage::seek_to_page_header(file)?;
    if !found {
        anyhow::bail!("Second ogg page not found");
    }
    OpusPage::from_reader(file)?;

    Ok((
        header_size,
        tonie_header,
        file_size,
        audio_size,
        sha1,
        opus_head_found,
        opus_version,
        channel_count,
        sample_rate,
        bitstream_serial_no,
    ))
}

pub fn get_audio_info(
    file: &mut File,
    sample_rate: u32,
    tonie_header: &TonieHeader,
    header_size: u64,
) -> Result<(
    u32,         // page_count
    bool,        // alignment_okay
    bool,        // page_size_okay
    u64,         // total_time
    Vec<String>, // chapters
)> {
    let mut chapter_granules = Vec::new();
    if tonie_header.chapterPages.contains(&0) {
        chapter_granules.push(0);
    }

    let mut alignment_okay = file.stream_position()? == (512 + 4 + header_size);
    let mut page_size_okay = true;
    let mut page_count = 2;

    let mut last_page = None;
    let mut found = OpusPage::seek_to_page_header(file)?;

    while found {
        page_count += 1;
        let page = OpusPage::from_reader(file)?;

        found = OpusPage::seek_to_page_header(file)?;
        if found && file.stream_position()? % 0x1000 != 0 {
            alignment_okay = false;
        }

        if page_size_okay && page_count > 3 && page.get_page_size() != 0x1000 && found {
            page_size_okay = false;
        }

        if tonie_header.chapterPages.contains(&page.page_no) {
            chapter_granules.push(page.granule_position);
        }

        last_page = Some(page.clone());
    }

    // Handle final granule position
    if let Some(page) = &last_page {
        chapter_granules.push(page.granule_position);
    } else {
        chapter_granules.push(0);
    }

    // Calculate chapter times
    let mut chapter_times = Vec::new();
    for i in 1..chapter_granules.len() {
        let length = chapter_granules[i] - chapter_granules[i - 1];
        chapter_times.push(granule_to_time_string(length, sample_rate));
    }

    // Calculate total time
    let total_time = if let Some(page) = &last_page {
        page.granule_position / sample_rate as u64
    } else {
        0
    };

    Ok((
        page_count,
        alignment_okay,
        page_size_okay,
        total_time,
        chapter_times,
    ))
}

fn format_time(timestamp: u64) -> String {
    let datetime = Utc
        .timestamp_opt(timestamp as i64, 0)
        .single()
        .unwrap_or_else(|| Utc::now());

    return datetime.format("%Y-%m-%d %H:%M:%S").to_string();
}

fn granule_to_time_string(granule: u64, sample_rate: u32) -> String {
    // TODO Using float division would increase precision of the "fraction" part
    let total_seconds = granule / sample_rate as u64;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds - (hours * 3600)) / 60;
    let seconds = total_seconds - (hours * 3600) - (minutes * 60);
    let fraction = (total_seconds * 100) % 100;

    return format!("{hours:02}:{minutes:02}:{seconds:02}.{fraction:02}");
}

pub fn split_to_opus_files(input: &Path, output_dir: Option<&Path>) -> Result<Vec<PathBuf>> {
    let mut file = File::open(input)?;

    // Read header
    let header_size = file.read_u32::<BigEndian>()?;
    let mut header_bytes = vec![0u8; header_size as usize];
    file.read_exact(&mut header_bytes)?;

    let tonie_header = TonieHeader::parse_from_bytes(&header_bytes)?;

    // Set up output path
    let input_abs_path = fs::canonicalize(input)?;
    let out_path = match output_dir {
        Some(path) => path.to_path_buf(),
        None => current_dir()?,
    };
    fs::create_dir_all(&out_path)?;

    // Read first three pages
    let found = OpusPage::seek_to_page_header(&mut file)?;
    if !found {
        anyhow::bail!("First ogg page not found");
    }
    let first_page = OpusPage::from_reader(&mut file)?;

    let found = OpusPage::seek_to_page_header(&mut file)?;
    if !found {
        anyhow::bail!("Second ogg page not found");
    }
    let second_page = OpusPage::from_reader(&mut file)?;

    let found = OpusPage::seek_to_page_header(&mut file)?;
    if !found {
        anyhow::bail!("End of file reached before finding a page");
    }
    let mut page = OpusPage::from_reader(&mut file)?;

    // Store the first and second pages' data
    let first_page_data = first_page.clone();
    let second_page_data = second_page.clone();

    let mut outfiles = Vec::new();
    for i in 0..tonie_header.chapterPages.len() {
        let chapter_index = i + 1;

        let end_page = if chapter_index < tonie_header.chapterPages.len() {
            tonie_header.chapterPages[chapter_index]
        } else {
            0
        };
        let mut granule = 0;

        // Create output filename template
        let file_stem = input_abs_path
            .file_stem()
            .ok_or_else(|| anyhow!("No file stem found"))?;
        let new_filename = format!("{chapter_index:02}_{}.opus", file_stem.to_string_lossy());
        let out_file_path = out_path.as_path().join(new_filename);
        
        outfiles.push(out_file_path.clone());

        let mut out_file = File::create(&out_file_path)?;
        let mut sha1 = Sha1::new();

        // Write the first and second pages' data
        first_page_data.write_page(&mut out_file, &mut sha1)?;
        second_page_data.write_page(&mut out_file, &mut sha1)?;

        let mut found = true;
        while found && (page.page_no < end_page || end_page == 0) {
            page.correct_values(granule)?;
            granule = page.granule_position;
            page.write_page(&mut out_file, &mut sha1)?;

            found = OpusPage::seek_to_page_header(&mut file)?;
            if found {
                page = OpusPage::from_reader(&mut file)?;
            }
        }
    }

    Ok(outfiles)
}

pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0u32;
    for &byte in data {
        let lookup_index = (((crc >> 24) ^ byte as u32) & 0xFF) as usize;
        crc = ((crc & 0xFFFFFF) << 8) ^ CRC_TABLE[lookup_index];
    }
    return crc;
}

lazy_static::lazy_static! {
    static ref CRC_TABLE: [u32; 256] = {
        let mut table = [0u32; 256];
        for i in 0..256 {
            let mut crc = (i as u32) << 24;
            for _ in 0..8 {
                crc = (crc << 1) ^ if (crc & 0x80000000) != 0 { 0x04C11DB7 } else { 0 };
            }
            table[i] = crc;
        }
        table
    };
}

use anyhow::{anyhow, Result};
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use protobuf::Message;
use sha1::{Digest, Sha1};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::SpooledTempFile;

use crate::opus_packet::OpusPacket;
use crate::ogg_page::OggPage;
use crate::tonie_header::tonie_header::TonieHeader;

const SAMPLE_RATE_KHZ: u32 = 48;

// Original OPUS_TAGS converted to Rust static arrays
static OPUS_TAGS: [&[u8]; 2] = [
    &[
        0x4f, 0x70, 0x75, 0x73, 0x54, 0x61, 0x67, 0x73, /* ... */
    ],
    &[
        0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, /* ... */
    ],
];

pub struct Converter;

pub trait ReadSeekSend: Read + Seek + Send {}
impl<T: Read + Seek + Send> ReadSeekSend for T {}

impl Converter {
    pub fn new() -> Self {
        Self
    }

    pub fn create_tonie_file(
        &self,
        output_file: PathBuf,
        input_files: Vec<PathBuf>,
        no_tonie_header: bool,
        user_timestamp: Option<String>,
        bitrate: u32,
        cbr: bool,
        ffmpeg: &str,
        opusenc: &str,
    ) -> Result<()> {
        let mut out_file = File::create(output_file)?;

        if !no_tonie_header {
            out_file.write_all(&vec![0u8; 0x1000])?;
        }

        let timestamp = match user_timestamp {
            Some(ts) => {
                if ts.starts_with("0x") {
                    u32::from_str_radix(&ts[2..], 16).unwrap()
                } else {
                    ts.parse::<u32>().unwrap()
                }
            }
            None => SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                .try_into()
                .unwrap(),
        };

        let mut sha1_hasher = Sha1::new();
        let mut template_page = None;
        let mut chapters: Vec<u32> = Vec::new();
        let mut total_granule = 0;
        let mut next_page_no = 2;
        let max_size = 0x1000;
        let mut other_size = 0xE00;

        let pad_len = (input_files.len() + 1).to_string().len();

        for (index, fname) in input_files.iter().enumerate() {
            println!(
                "[{:0width$}/{}] {}",
                index + 1,
                input_files.len(),
                fname.display(),
                width = pad_len
            );

            let last_track = index == input_files.len() - 1;

            let mut handle: Box<dyn ReadSeekSend> =
                if fname.extension().unwrap_or_default() == "opus" {
                    Box::new(File::open(fname)?)
                } else {
                    self.get_opus_tempfile(ffmpeg, opusenc, fname, bitrate, !cbr)?
                };

            if next_page_no == 2 {
                self.copy_first_and_second_page(&mut handle, &mut out_file, timestamp, &mut sha1_hasher)?;
            } else {
                other_size = max_size;
                self.skip_first_two_pages(&mut handle)?;
            }

            let pages = self.read_all_remaining_pages(&mut handle)?;

            if template_page.is_none() {
                template_page = Some(OggPage::from_page(&pages[0]));
                template_page.as_mut().unwrap().serial_no = timestamp;
            }

            if next_page_no == 2 {
                chapters.push(0);
            } else {
                chapters.push(next_page_no);
            }

            let new_pages = self.resize_pages(
                pages,
                max_size,
                other_size,
                template_page.as_ref().unwrap(),
                total_granule,
                next_page_no,
                last_track,
            )?;

            for new_page in &new_pages {
                new_page.write_page(&mut out_file, Some(&mut sha1_hasher))?;
            }

            if let Some(last_page) = new_pages.last() {
                total_granule = last_page.granule_position;
                next_page_no = last_page.page_no + 1;
            }
        }

        if !no_tonie_header {
            self.fix_tonie_header(&mut out_file, chapters, timestamp, &mut sha1_hasher)?;
        }

        Ok(())
    }

    fn fix_tonie_header(
        &self,
        out_file: &mut File,
        chapters: Vec<u32>,
        timestamp: u32,
        sha_hasher: &mut Sha1,
    ) -> Result<()> {
        let mut tonie_header = TonieHeader {
            dataHash: sha_hasher.finalize_reset().to_vec(),
            dataLength: (out_file.stream_position()? - 0x1000) as u32,
            timestamp,
            chapterPages: chapters,
            padding: vec![0; 0x100],
            special_fields: Default::default(),
        };

        let header = tonie_header.write_to_bytes()?;
        let pad = 0xFFC - header.len() + 0x100;
        tonie_header.padding = vec![0; pad];
        let header = tonie_header.write_to_bytes()?;

        out_file.seek(SeekFrom::Start(0))?;
        out_file.write_u32::<BigEndian>(header.len() as u32)?;
        out_file.write_all(&header)?;

        Ok(())
    }

    fn copy_first_and_second_page(
        &self,
        in_file: &mut impl ReadSeekSend,
        out_file: &mut File,
        timestamp: u32,
        sha_hasher: &mut Sha1,
    ) -> Result<()> {
        if !OggPage::seek_to_page_header(in_file)? {
            return Err(anyhow!("First ogg page not found"));
        }
        let mut page = OggPage::from_reader(in_file)?;
        page.serial_no = timestamp;
        page.checksum = page.calc_checksum();
        self.check_identification_header(&page)?;
        page.write_page(out_file, Some(sha_hasher))?;

        if !OggPage::seek_to_page_header(in_file)? {
            return Err(anyhow!("Second ogg page not found"));
        }

        let mut page = OggPage::from_reader(in_file)?;
        page.serial_no = timestamp;
        page.checksum = page.calc_checksum();
        page = self.prepare_opus_tags(page)?;
        page.write_page(out_file, Some(sha_hasher))?;

        Ok(())
    }
    fn skip_first_two_pages(&self, in_file: &mut impl ReadSeekSend) -> Result<()> {
        if !OggPage::seek_to_page_header(in_file)? {
            return Err(anyhow!("First ogg page not found"));
        }
        let page = OggPage::from_reader(in_file)?;
        self.check_identification_header(&page)?;

        if !OggPage::seek_to_page_header(in_file)? {
            return Err(anyhow!("Second ogg page not found"));
        }
        OggPage::from_reader(in_file)?;

        Ok(())
    }

    fn read_all_remaining_pages(&self, in_file: &mut impl ReadSeekSend) -> Result<Vec<OggPage>> {
        let mut remaining_pages = Vec::new();

        while OggPage::seek_to_page_header(in_file)? {
            remaining_pages.push(OggPage::from_reader(in_file)?);
        }

        Ok(remaining_pages)
    }

    fn resize_pages(
        &self,
        mut old_pages: Vec<OggPage>,
        max_page_size: usize,
        first_page_size: usize,
        template_page: &OggPage,
        last_granule: u64,
        start_no: u32,
        set_last_page_flag: bool,
    ) -> Result<Vec<OggPage>> {
        let mut new_pages = Vec::new();
        let mut current_page = None;
        let mut page_no = start_no;
        let mut max_size = first_page_size;

        let mut new_page = OggPage::from_page(template_page);
        new_page.page_no = page_no;

        while !old_pages.is_empty() || current_page.is_some() {
            let page = if let Some(p) = current_page.take() {
                p
            } else {
                old_pages.remove(0)
            };

            let size = page.get_size_of_first_opus_packet();
            let seg_count = page.get_segment_count_of_first_opus_packet();

            if (size + seg_count + new_page.get_page_size() <= max_size)
                && (new_page.segments.len() + seg_count < 256)
            {
                for _ in 0..seg_count {
                    if let Some(segment) = page.segments.first().cloned() {
                        new_page.segments.push(segment);
                    }
                }
                if !page.segments.is_empty() {
                    current_page = Some(page);
                }
            } else {
                new_page.pad(max_size, Default::default())?;
                new_page.correct_values(last_granule)?;
                new_pages.push(new_page);

                new_page = OggPage::from_page(template_page);
                page_no += 1;
                new_page.page_no = page_no;
                max_size = max_page_size;
            }
        }

        if !new_page.segments.is_empty() {
            if set_last_page_flag {
                new_page.page_type = 4;
            }
            new_page.pad(max_size, Default::default())?;
            new_page.correct_values(last_granule)?;
            new_pages.push(new_page);
        }

        Ok(new_pages)
    }

    fn prepare_opus_tags(&self, mut page: OggPage) -> Result<OggPage> {
        page.segments.clear();

        let mut segment = OpusPacket::new::<std::io::Empty>(None, 0, 0, false)
            .expect("Failed to create OpusPacket");
        segment.size = OPUS_TAGS[0].len() as i32;
        segment.data = OPUS_TAGS[0].to_vec();
        segment.spanning_packet = true;
        segment.first_packet = true;
        page.segments.push(segment);

        let mut segment = OpusPacket::new::<std::io::Empty>(None, 0, 0, false)
            .expect("Failed to create OpusPacket");
        segment.size = OPUS_TAGS[1].len() as i32;
        segment.data = OPUS_TAGS[1].to_vec();
        segment.spanning_packet = false;
        segment.first_packet = false;
        page.segments.push(segment);

        page.correct_values(0)?;
        return Ok(page)
    }

    fn check_identification_header(&self, page: &OggPage) -> Result<()> {
        if let Some(segment) = page.segments.first() {
            let data = &segment.data[..18];
            let magic = &data[..8];
            let version = data[8];
            let channels = data[9];
            let sample_rate = byteorder::LittleEndian::read_u32(&data[12..16]);

            if magic != b"OpusHead" {
                return Err(anyhow!("Invalid opus file?"));
            }
            if version != 1 {
                return Err(anyhow!("Invalid opus file?"));
            }
            if channels != 2 {
                return Err(anyhow!("Only stereo tracks are supported"));
            }
            if sample_rate != SAMPLE_RATE_KHZ * 1000 {
                return Err(anyhow!("Sample rate needs to be 48 kHz"));
            }
        }
        Ok(())
    }

    pub fn get_opus_tempfile(
        &self,
        ffmpeg_binary: &str,
        opus_binary: &str,
        filename: &PathBuf,
        bitrate: u32,
        vbr: bool,
    ) -> Result<Box<SpooledTempFile>> {
        let vbr_parameter = if !vbr { "--hard-cbr" } else { "--vbr" };

        let ffmpeg_process = Command::new(ffmpeg_binary)
            .args([
                "-hide_banner",
                "-loglevel",
                "warning",
                "-i",
                filename.to_str().unwrap(),
                "-f",
                "wav",
                "-ar",
                "48000",
                "-",
            ])
            .stdout(Stdio::piped())
            .spawn()?;

        let opusenc_process = Command::new(opus_binary)
            .args([
                "--quiet",
                vbr_parameter,
                "--bitrate",
                &bitrate.to_string(),
                "-",
                "-",
            ])
            .stdin(ffmpeg_process.stdout.unwrap())
            .stdout(Stdio::piped())
            // .stderr(Stdio::null())
            .spawn()?;

        let mut tmp_file = SpooledTempFile::new(50 * 1024 * 1024);

        // Await processes to finish
        let opusenc_status = opusenc_process.wait_with_output()?;
        if !opusenc_status.status.success() {
            return Err(anyhow!("opusenc failed: {}", opusenc_status.status));
        }
        tmp_file.write(&opusenc_status.stdout).ok();
        tmp_file.seek(SeekFrom::Start(0))?;

        Ok(Box::new(tmp_file))
    }
}

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::SpooledTempFile;
use sha1::{Sha1, Digest};
use byteorder::{BigEndian, LittleEndian, ReadBytesExt, WriteBytesExt};
use log::info;
use std::fmt;

use crate::opus_page::OpusPage;

const SAMPLE_RATE_KHZ: u32 = 48;

// Original OPUS_TAGS converted to Rust static arrays
static OPUS_TAGS: [&[u8]; 2] = [
    &[0x4f, 0x70, 0x75, 0x73, 0x54, 0x61, 0x67, 0x73, /* ... */],
    &[0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, /* ... */],
];

pub struct Converter;

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
    ) -> io::Result<()> {
        let mut out_file = File::create(output_file)?;
        
        if !no_tonie_header {
            out_file.write_all(&vec![0u8; 0x1000])?;
        }

        let timestamp = match user_timestamp {
            Some(ts) => {
                if ts.starts_with("0x") {
                    u64::from_str_radix(&ts[2..], 16).unwrap()
                } else {
                    ts.parse::<u64>().unwrap()
                }
            },
            None => SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        let mut sha1 = Sha1::new();
        let mut template_page = None;
        let mut chapters = Vec::new();
        let mut total_granule = 0;
        let mut next_page_no = 2;
        let max_size = 0x1000;
        let mut other_size = 0xE00;

        let pad_len = (input_files.len() + 1).to_string().len();
        
        for (index, fname) in input_files.iter().enumerate() {
            println!("[{:0width$}/{}] {}", 
                    index + 1, 
                    input_files.len(), 
                    fname.display(),
                    width = pad_len);
            
            let last_track = index == input_files.len() - 1;

            let mut handle = if fname.extension().unwrap_or_default() == "opus" {
                Box::new(File::open(fname)?) as Box<dyn Read + Send>
            } else {
                self.get_opus_tempfile(ffmpeg, opusenc, fname, bitrate, !cbr)?
            };

            if next_page_no == 2 {
                self.copy_first_and_second_page(&mut handle, &mut out_file, timestamp, &mut sha1)?;
            } else {
                other_size = max_size;
                self.skip_first_two_pages(&mut handle)?;
            }

            let pages = self.read_all_remaining_pages(&mut handle)?;

            if template_page.is_none() {
                template_page = Some(OpusPage::from_page(&pages[0]));
                template_page.as_mut().unwrap().serial_no = timestamp;
            }

            if next_page_no == 2 {
                chapters.push(0);
            } else {
                chapters.push(next_page_no as i32);
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
                new_page.write_page(&mut out_file, &mut sha1)?;
            }

            if let Some(last_page) = new_pages.last() {
                total_granule = last_page.granule_position;
                next_page_no = last_page.page_no + 1;
            }
        }

        if !no_tonie_header {
            self.fix_tonie_header(&mut out_file, &chapters, timestamp, &mut sha1)?;
        }

        Ok(())
    }

    fn fix_tonie_header(
        &self,
        out_file: &mut File,
        chapters: &[i32],
        timestamp: u64,
        sha: &mut Sha1,
    ) -> io::Result<()> {
        let mut tonie_header = TonieHeader {
            data_hash: sha.finalize_reset().to_vec(),
            data_length: out_file.stream_position()? - 0x1000,
            timestamp,
            chapter_pages: chapters.to_vec(),
            padding: vec![0; 0x100],
        };

        let header = tonie_header.encode_to_vec();
        let pad = 0xFFC - header.len() + 0x100;
        tonie_header.padding = vec![0; pad];
        let header = tonie_header.encode_to_vec();

        out_file.seek(SeekFrom::Start(0))?;
        out_file.write_u32::<BigEndian>(header.len() as u32)?;
        out_file.write_all(&header)?;

        Ok(())
    }

    fn copy_first_and_second_page(
        &self,
        in_file: &mut dyn Read,
        out_file: &mut File,
        timestamp: u64,
        sha: &mut Sha1,
    ) -> io::Result<()> {
        if !OpusPage::seek_to_page_header(in_file)? {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "First ogg page not found"));
        }
        let mut page = OpusPage::read_page(in_file)?;
        page.serial_no = timestamp;
        page.checksum = page.calc_checksum();
        self.check_identification_header(&page)?;
        page.write_page(out_file, sha)?;

        if !OpusPage::seek_to_page_header(in_file)? {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Second ogg page not found"));
        }
        let mut page = OpusPage::read_page(in_file)?;
        page.serial_no = timestamp;
        page.checksum = page.calc_checksum();
        page = self.prepare_opus_tags(page);
        page.write_page(out_file, sha)?;

        Ok(())
    }

    fn skip_first_two_pages(&self, in_file: &mut dyn Read) -> io::Result<()> {
        if !OpusPage::seek_to_page_header(in_file)? {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "First ogg page not found"));
        }
        let page = OpusPage::read_page(in_file)?;
        self.check_identification_header(&page)?;

        if !OpusPage::seek_to_page_header(in_file)? {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Second ogg page not found"));
        }
        OpusPage::read_page(in_file)?;

        Ok(())
    }

    fn read_all_remaining_pages(&self, in_file: &mut dyn Read) -> io::Result<Vec<OpusPage>> {
        let mut remaining_pages = Vec::new();

        while OpusPage::seek_to_page_header(in_file)? {
            remaining_pages.push(OpusPage::read_page(in_file)?);
        }
        
        Ok(remaining_pages)
    }

    fn resize_pages(
        &self,
        mut old_pages: Vec<OpusPage>,
        max_page_size: usize,
        first_page_size: usize,
        template_page: &OpusPage,
        last_granule: i64,
        start_no: u32,
        set_last_page_flag: bool,
    ) -> io::Result<Vec<OpusPage>> {
        let mut new_pages = Vec::new();
        let mut current_page = None;
        let mut page_no = start_no;
        let mut max_size = first_page_size;

        let mut new_page = OpusPage::from_page(template_page);
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
                && (new_page.segments.len() + seg_count < 256) {
                for _ in 0..seg_count {
                    if let Some(segment) = page.segments.first().cloned() {
                        new_page.segments.push(segment);
                    }
                }
                if !page.segments.is_empty() {
                    current_page = Some(page);
                }
            } else {
                new_page.pad(max_size);
                new_page.correct_values(last_granule);
                new_pages.push(new_page);

                new_page = OpusPage::from_page(template_page);
                page_no += 1;
                new_page.page_no = page_no;
                max_size = max_page_size;
            }
        }

        if !new_page.segments.is_empty() {
            if set_last_page_flag {
                new_page.page_type = 4;
            }
            new_page.pad(max_size);
            new_page.correct_values(last_granule);
            new_pages.push(new_page);
        }

        Ok(new_pages)
    }

    fn prepare_opus_tags(&self, mut page: OpusPage) -> OpusPage {
        page.segments.clear();
        
        let mut segment = OpusPacket::new();
        segment.size = OPUS_TAGS[0].len();
        segment.data = OPUS_TAGS[0].to_vec();
        segment.spanning_packet = true;
        segment.first_packet = true;
        page.segments.push(segment);

        let mut segment = OpusPacket::new();
        segment.size = OPUS_TAGS[1].len();
        segment.data = OPUS_TAGS[1].to_vec();
        segment.spanning_packet = false;
        segment.first_packet = false;
        page.segments.push(segment);
        
        page.correct_values(0);
        page
    }

    fn check_identification_header(&self, page: &OpusPage) -> io::Result<()> {
        if let Some(segment) = page.segments.first() {
            let data = &segment.data[..18];
            let magic = &data[..8];
            let version = data[8];
            let channels = data[9];
            let sample_rate = LittleEndian::read_u32(&data[12..16]);

            if magic != b"OpusHead" {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid opus file?"));
            }
            if version != 1 {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid opus file?"));
            }
            if channels != 2 {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "Only stereo tracks are supported"));
            }
            if sample_rate != SAMPLE_RATE_KHZ * 1000 {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "Sample rate needs to be 48 kHz"));
            }
        }
        Ok(())
    }

    fn get_opus_tempfile(
        &self,
        ffmpeg_binary: &str,
        opus_binary: &str,
        filename: &PathBuf,
        bitrate: u32,
        vbr: bool,
    ) -> io::Result<Box<dyn Read + Send>> {
        let vbr_parameter = if !vbr { "--hard-cbr" } else { "--vbr" };

        let mut ffmpeg_process = Command::new(ffmpeg_binary)
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

        let ffmpeg_stdout = ffmpeg_process.stdout.take().unwrap();

        let mut opusenc_process = Command::new(opus_binary)
            .args([
                "--quiet",
                vbr_parameter,
                "--bitrate",
                &bitrate.to_string(),
                "-",
                "-",
            ])
            .stdin(ffmpeg_stdout)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;

        let mut tmp_file = SpooledTempFile::new();
        if let Some(mut stdout) = opusenc_process.stdout.take() {
            io::copy(&mut stdout, &mut tmp_file)?;
        }
        tmp_file.seek(SeekFrom::Start(0))?;

        Ok(Box::new(tmp_file))
    }
}
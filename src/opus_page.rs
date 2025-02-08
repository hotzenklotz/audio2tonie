use std::io::{Read, Seek, SeekFrom};
use std::convert::TryInto;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use sha1::Sha1;
use crate::opus_packet::OpusPacket;
use crate::utils::crc32;

// Constants
const ONLY_CONVERT_FRAMEPACKING: i32 = -1;
const OTHER_PACKET_NEEDED: i32 = -2;
const DO_NOTHING: i32 = -3;
const TOO_MANY_SEGMENTS: i32 = -4;

// Main struct definition
pub struct OpusPage {
    pub version: u8,
    pub page_type: Option<u8>,
    pub granule_position: i64,
    pub serial_no: u32,
    pub page_no: u32,
    pub checksum: Option<u32>,
    pub segment_count: u8,
    pub segments: Vec<OpusPacket>,
}

impl OpusPage {
    pub fn new() -> Self {
        OpusPage {
            version: 0,
            page_type: None,
            granule_position: 0,
            serial_no: 0,
            page_no: 0,
            checksum: None,
            segment_count: 0,
            segments: Vec::new(),
        }
    }

    pub fn from_reader<R: Read>(reader: &mut R) -> std::io::Result<Self> {
        let mut page = OpusPage::new();
        page.parse_header(reader)?;
        page.parse_segments(reader)?;
        Ok(page)
    }

    fn parse_header<R: Read>(&mut self, reader: &mut R) -> std::io::Result<()> {
        let mut header = vec![0u8; 27];
        reader.read_exact(&mut header)?;

        // Skip first 4 bytes as they're the "OggS" magic number
        let mut cursor = std::io::Cursor::new(&header[4..]);
        
        self.version = cursor.read_u8()?;
        self.page_type = Some(cursor.read_u8()?);
        self.granule_position = cursor.read_i64::<LittleEndian>()?;
        self.serial_no = cursor.read_u32::<LittleEndian>()?;
        self.page_no = cursor.read_u32::<LittleEndian>()?;
        self.checksum = Some(cursor.read_u32::<LittleEndian>()?);
        self.segment_count = cursor.read_u8()?;

        Ok(())
    }

    fn parse_segments<R: Read>(&mut self, reader: &mut R) -> std::io::Result<()> {
        let mut table = vec![0u8; self.segment_count as usize];
        reader.read_exact(&mut table)?;

        let mut last_length: i32 = -1;
        let dont_parse_info = self.page_no == 0 || self.page_no == 1;

        self.segments.clear();
        for &length in table.iter() {
            let segment = OpusPacket::from_reader(
                reader,
                length as i32,
                last_length,
                dont_parse_info,
            )?;
            last_length = length as i32;
            self.segments.push(segment);
        }

        if self.segments.last().map_or(false, |s| s.spanning_packet) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Found an opus packet spanning ogg pages. This is not supported yet."
            ));
        }

        Ok(())
    }

    pub fn correct_values(&mut self, last_granule: i64) -> std::io::Result<()> {
        if self.segments.len() > 255 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Too many segments: {} - max 255 allowed", self.segments.len())
            ));
        }

        let mut granule = 0;
        if self.page_no != 0 && self.page_no != 1 {
            for segment in &self.segments {
                if segment.first_packet {
                    granule += segment.granule;
                }
            }
        }

        self.granule_position = last_granule + granule;
        self.segment_count = self.segments.len() as u8;
        self.checksum = Some(self.calc_checksum());

        Ok(())
    }

    fn calc_checksum(&self) -> u32 {
        let mut data = Vec::new();
        data.extend_from_slice(b"OggS");
        
        // Pack header data
        data.push(self.version);
        data.push(self.page_type.unwrap_or(0));
        data.extend_from_slice(&self.granule_position.to_le_bytes());
        data.extend_from_slice(&self.serial_no.to_le_bytes());
        data.extend_from_slice(&self.page_no.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes()); // Checksum placeholder
        data.push(self.segment_count);

        // Add segment table
        for segment in &self.segments {
            data.push(segment.size as u8);
        }

        // Add segment data
        for segment in &self.segments {
            data.extend_from_slice(&segment.data);
        }

        crc32(&data)
    }

    pub fn get_page_size(&self) -> usize {
        let mut size = 27 + self.segments.len();
        for segment in &self.segments {
            size += segment.data.len();
        }
        size
    }

    pub fn get_size_of_first_opus_packet(&self) -> usize {
        if self.segments.is_empty() {
            return 0;
        }

        let mut segment_size = self.segments[0].size;
        let mut size = segment_size;
        let mut i = 1;

        while segment_size == 255 && i < self.segments.len() {
            segment_size = self.segments[i].size;
            size += segment_size;
            i += 1;
        }

        size as usize
    }

    pub fn get_segment_count_of_first_opus_packet(&self) -> usize {
        if self.segments.is_empty() {
            return 0;
        }

        let mut segment_size = self.segments[0].size;
        let mut count = 1;

        while segment_size == 255 && count < self.segments.len() {
            segment_size = self.segments[count].size;
            count += 1;
        }

        count
    }

    pub fn insert_empty_segment(
        &mut self,
        index_after: usize,
        spanning_packet: bool,
        first_packet: bool,
    ) {
        let mut segment = OpusPacket::new();
        segment.first_packet = first_packet;
        segment.spanning_packet = spanning_packet;
        segment.size = 0;
        segment.data = Vec::new();
        self.segments.insert(index_after + 1, segment);
    }

    pub fn get_opus_packet_size(&self, seg_start: usize) -> usize {
        let mut size = self.segments[seg_start].data.len();
        let mut current = seg_start + 1;

        while current < self.segments.len() && !self.segments[current].first_packet {
            size += self.segments[current].size as usize;
            current += 1;
        }

        size
    }

    pub fn get_segment_count_of_packet_at(&self, seg_start: usize) -> usize {
        let mut seg_end = seg_start + 1;
        while seg_end < self.segments.len() && !self.segments[seg_end].first_packet {
            seg_end += 1;
        }
        seg_end - seg_start
    }

    pub fn redistribute_packet_data_at(&mut self, seg_start: usize, pad_count: usize) {
        let seg_count = self.get_segment_count_of_packet_at(seg_start);
        let mut full_data = Vec::new();

        // Collect all data
        for i in 0..seg_count {
            full_data.extend_from_slice(&self.segments[seg_start + i].data);
        }
        full_data.extend(vec![0u8; pad_count]);
        let size = full_data.len();

        if size < 255 {
            self.segments[seg_start].size = size as i32;
            self.segments[seg_start].data = full_data;
            return;
        }

        let needed_seg_count = if size % 255 == 0 {
            (size / 255) + 1
        } else {
            (size + 254) / 255
        };

        let segments_to_create = needed_seg_count - seg_count;
        for i in 0..segments_to_create {
            self.insert_empty_segment(
                seg_start + seg_count + i,
                i != (segments_to_create - 1),
                false,
            );
        }

        // Redistribute data
        for i in 0..needed_seg_count {
            let chunk_size = std::cmp::min(255, full_data.len());
            let chunk = full_data[..chunk_size].to_vec();
            self.segments[seg_start + i].data = chunk;
            self.segments[seg_start + i].size = chunk.len() as i32;
            full_data = full_data[chunk_size..].to_vec();
        }
        
        assert!(full_data.is_empty());
    }

    pub fn convert_packet_to_framepacking_three_and_pad(
        &mut self,
        seg_start: usize,
        pad: bool,
        count: usize,
    ) {
        assert!(self.segments[seg_start].first_packet);
        self.segments[seg_start].convert_to_framepacking_three();
        if pad {
            self.segments[seg_start].set_pad_count(count);
        }
        self.redistribute_packet_data_at(seg_start, count);
    }

    pub fn calc_actual_padding_value(&self, seg_start: usize, bytes_needed: i32) -> i32 {
        assert!(bytes_needed >= 0, "Page is already too large! Something went wrong.");

        let seg_end = seg_start + self.get_segment_count_of_packet_at(seg_start);
        let size_of_last_segment = self.segments[seg_end - 1].size;
        let convert_framepacking_needed = self.segments[seg_start].framepacking != Some(3);

        if bytes_needed == 0 {
            return DO_NOTHING;
        }

        if (bytes_needed + size_of_last_segment) % 255 == 0 {
            return OTHER_PACKET_NEEDED;
        }

        if bytes_needed == 1 {
            return if convert_framepacking_needed {
                ONLY_CONVERT_FRAMEPACKING
            } else {
                0
            };
        }

        let mut new_segments_needed = 0;
        if bytes_needed + size_of_last_segment >= 255 {
            let mut tmp_count = bytes_needed + size_of_last_segment - 255;
            while tmp_count >= 0 {
                tmp_count = tmp_count - 255 - 1;
                new_segments_needed += 1;
            }
        }

        if new_segments_needed + self.segments.len() > 255 {
            return TOO_MANY_SEGMENTS;
        }

        if (bytes_needed + size_of_last_segment) % 255 == (new_segments_needed - 1) {
            return OTHER_PACKET_NEEDED;
        }

        let mut packet_bytes_needed = bytes_needed - new_segments_needed;

        if packet_bytes_needed == 1 {
            return if convert_framepacking_needed {
                ONLY_CONVERT_FRAMEPACKING
            } else {
                0
            };
        }

        if convert_framepacking_needed {
            packet_bytes_needed -= 1; // frame_count_byte
        }
        packet_bytes_needed -= 1; // padding_count_data is at least 1 byte

        let size_of_padding_count_data = std::cmp::max(
            1,
            ((packet_bytes_needed as f64) / 254.0).ceil() as i32
        );
        let check_size = ((packet_bytes_needed - size_of_padding_count_data + 1) as f64 / 254.0)
            .ceil() as i32;

        if check_size != size_of_padding_count_data {
            OTHER_PACKET_NEEDED
        } else {
            packet_bytes_needed - size_of_padding_count_data + 1
        }
    }

    pub fn pad(&mut self, pad_to: usize, idx_offset: Option<usize>) {
        let mut idx = idx_offset.unwrap_or_else(|| self.segments.len() - 1);

        while !self.segments[idx].first_packet {
            idx = idx.checked_sub(1).expect("Could not find begin of last packet!");
        }

        let pad_count = pad_to as i32 - self.get_page_size() as i32;
        let actual_padding = self.calc_actual_padding_value(idx, pad_count);

        match actual_padding {
            DO_NOTHING => return,
            ONLY_CONVERT_FRAMEPACKING => {
                self.convert_packet_to_framepacking_three_and_pad(idx, false, 0);
                return;
            }
            OTHER_PACKET_NEEDED => {
                self.pad_one_byte();
                self.pad(pad_to, None);
                return;
            }
            TOO_MANY_SEGMENTS => {
                self.pad(pad_to - (pad_count as usize / 2), Some(idx - 1));
                self.pad(pad_to, None);
                return;
            }
            _ => {
                self.convert_packet_to_framepacking_three_and_pad(idx, true, actual_padding as usize);
                assert_eq!(self.get_page_size(), pad_to);
            }
        }
    }

    pub fn pad_one_byte(&mut self) -> std::io::Result<()> {
        let mut i = 0;
        while i < self.segments.len() {
            if self.segments[i].first_packet 
                && !self.segments[i].padding
                && self.get_opus_packet_size(i) % 255 < 254 {
                break;
            }
            i += 1;
            if i >= self.segments.len() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Page seems impossible to pad correctly"
                ));
            }
        }

        if self.segments[i].framepacking == Some(3) {
            self.convert_packet_to_framepacking_three_and_pad(i, true, 0);
        } else {
            self.convert_packet_to_framepacking_three_and_pad(i, false, 0);
        }

        Ok(())
    }

    pub fn write_page<W: std::io::Write>(
        &self,
        writer: &mut W,
        sha1: Option<&mut Sha1>,
    ) -> std::io::Result<()> {
        let mut data = Vec::new();
        data.extend_from_slice(b"OggS");
        
        // Write header
        data.push(self.version);
        data.push(self.page_type.unwrap_or(0));
        data.extend_from_slice(&self.granule_position.to_le_bytes());
        data.extend_from_slice(&self.serial_no.to_le_bytes());
        data.extend_from_slice(&self.page_no.to_le_bytes());
        data.extend_from_slice(&self.checksum.unwrap_or(0).to_le_bytes());
        data.push(self.segment_count);

        // Write segment table
        for segment in &self.segments {
            data.push(segment.size as u8);
        }

        if let Some(sha1) = sha1 {
            sha1.update(&data);
        }
        writer.write_all(&data)?;

        // Write segment data
        for segment in &self.segments {
            if let Some(sha1) = sha1 {
                sha1.update(&segment.data);
            }
            writer.write_all(&segment.data)?;
        }

        Ok(())
    }

    pub fn from_page(other_page: &OpusPage) -> Self {
        OpusPage {
            version: other_page.version,
            page_type: other_page.page_type,
            granule_position: other_page.granule_position,
            serial_no: other_page.serial_no,
            page_no: other_page.page_no,
            checksum: Some(0),
            segment_count: 0,
            segments: Vec::new(),
        }
    }

    pub fn seek_to_page_header<R: Read + Seek>(reader: &mut R) -> std::io::Result<bool> {
        let current_pos = reader.stream_position()?;
        reader.seek(SeekFrom::End(0))?;
        let size = reader.stream_position()?;
        reader.seek(SeekFrom::Start(current_pos))?;

        let mut five_bytes = vec![0u8; 5];
        reader.read_exact(&mut five_bytes)?;

        while reader.stream_position()? + 5 < size {
            if five_bytes == b"OggS\x00" {
                reader.seek(SeekFrom::Current(-5))?;
                return Ok(true);
            }
            reader.seek(SeekFrom::Current(-4))?;
            reader.read_exact(&mut five_bytes)?;
        }

        Ok(false)
    }
}
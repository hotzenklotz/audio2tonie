use anyhow::{anyhow, Result};
use byteorder::ReadBytesExt;
use std::io::{Cursor, Read, Write};

const SAMPLE_RATE_KHZ: u32 = 48;

#[derive(Debug, Default, Clone)]
pub struct OpusPacket {
    pub config_value: Option<u8>,
    pub stereo: Option<u8>,
    pub framepacking: i8,
    pub padding: Option<u32>,
    pub frame_count: Option<u32>,
    pub frame_size: Option<f32>,
    pub granule: u64,
    pub size: i32,
    pub data: Vec<u8>,
    pub spanning_packet: bool,
    pub first_packet: bool,
}

impl OpusPacket {
    pub fn new<R: Read>(
        filehandle: Option<&mut R>,
        size: i32,
        last_size: i32,
        dont_parse_info: bool,
    ) -> Result<Self> {
        let mut packet = OpusPacket {
            size,
            ..Default::default()
        };

        if let Some(fh) = filehandle {
            packet.size = size;
            let mut buf = vec![0u8; size as usize];
            fh.read_exact(&mut buf)?;
            packet.data = buf;
            packet.framepacking = -1;
            packet.spanning_packet = size == 255;
            packet.first_packet = last_size != 255;

            if packet.first_packet && !dont_parse_info {
                packet.parse_segment_info()?;
            }
        }

        Ok(packet)
    }

    fn get_frame_count(&self) -> u32 {
        return match self.framepacking {
            0 => 1,
            1 | 2 => 2,
            3 => {
                if self.data.len() >= 2 {
                    (self.data[1] & 63) as u32
                } else {
                    0
                }
            }
            _ => 0,
        };
    }

    fn get_padding(&self) -> Result<u32> {
        if self.framepacking != 3 {
            return Ok(0);
        }

        if self.data.len() < 3 {
            anyhow::bail!("Data too short to determine padding");
        }

        let mut cursor = Cursor::new(&self.data[1..3]);
        let byte1 = cursor.read_u8()?;
        let byte2 = cursor.read_u8()?;

        let is_padded = (byte1 >> 6) & 1;
        if is_padded == 0 {
            return Ok(0);
        }

        let mut total_padding = byte2 as u32;
        let mut i = 3;
        let mut padding: u8 = byte2;

        while padding == 255 {
            if self.data.len() <= i {
                anyhow::bail!("Data too short to read additional padding");
            }
            padding = self.data[i];
            total_padding += (padding - 1) as u32;
            i += 1;
        }

        Ok(total_padding)
    }

    fn get_frame_size(&self) -> Result<f32, String> {
        match self.config_value {
            Some(16..=31) => {
                match self.config_value.unwrap() {
                    16 | 20 | 24 | 28 => Ok(2.5),
                    17 | 21 | 25 | 29 => Ok(5.0),
                    18 | 22 | 26 | 30 => Ok(10.0),
                    19 | 23 | 27 | 31 => Ok(20.0),
                    _ => unreachable!(), // We already checked the range
                }
            }
            Some(val) => Err(format!(
                "Found config value {} in opus packet, but CELT-only encodings (16-31) are required by the box.\n\
                Please encode your input files accordingly or fix your encoding pipeline to do so.\n\
                Did you built libopus with custom modes support?",
                val
            )),
            None => Err("Config value not set".to_string()),
        }
    }

    fn parse_segment_info(&mut self) -> Result<()> {
        if self.data.is_empty() {
            return Err(anyhow!("Empty data"));
        }

        let byte = self.data[0];
        self.config_value = Some(byte >> 3);
        self.stereo = Some((byte & 4) >> 2);
        self.framepacking = (byte & 3) as i8;
        self.padding = match self.get_padding() {
            Ok(padding) => Some(padding),
            Err(_) => None,
        };
        self.frame_count = Some(self.get_frame_count());

        match self.get_frame_size() {
            Ok(frame_size) => {
                self.frame_size = Some(frame_size);
                // TODO check this!
                self.granule = (frame_size
                    * self.frame_count.unwrap_or(0) as f32
                    * SAMPLE_RATE_KHZ as f32) as u64;
                Ok(())
            }

            Err(e) => Err(anyhow!(e)),
        }
    }

    pub fn write<W: Write>(&self, mut filehandle: W) -> Result<()> {
        if !self.data.is_empty() {
            filehandle.write_all(&self.data)?;
        }
        Ok(())
    }

    pub fn convert_to_framepacking_three(&mut self) {
        if self.framepacking == 3 {
            return;
        }

        let mut toc_byte = self.data[0];
        toc_byte |= 0b11;

        let mut frame_count_byte = self.frame_count.unwrap_or(0) as u8;
        if self.framepacking == 2 {
            frame_count_byte |= 0b10000000; // vbr
        }

        let mut new_data = Vec::with_capacity(self.data.len() + 1);
        new_data.push(toc_byte);
        new_data.push(frame_count_byte);
        new_data.extend_from_slice(&self.data[1..]);

        self.data = new_data;
        self.framepacking = 3;
    }

    pub fn set_pad_count(&mut self, count: u32) -> Result<()> {
        if self.framepacking != 3 {
            return Err(anyhow!("Only code 3 packets can contain padding!"));
        }
        if self.padding != Some(0) {
            return Err(anyhow!("Packet already padded. Not supported yet!"));
        }

        let frame_count_byte = self.data[1] | 0b01000000;

        let mut pad_count_data = Vec::new();
        let mut val = count;
        while val > 254 {
            pad_count_data.push(0xff);
            val -= 254;
        }
        pad_count_data.push(val as u8);

        let mut new_data = Vec::with_capacity(self.data.len() + pad_count_data.len());
        new_data.push(self.data[0]);
        new_data.push(frame_count_byte);
        new_data.extend_from_slice(&pad_count_data);
        new_data.extend_from_slice(&self.data[2..]);

        self.data = new_data;
        Ok(())
    }
}

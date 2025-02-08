use std::io::{Read, Write};
use std::io;

const SAMPLE_RATE_KHZ: u32 = 48;

#[derive(Debug, Default)]
pub struct OpusPacket {
    config_value: Option<u8>,
    stereo: Option<u8>,
    framepacking: Option<u8>,
    padding: Option<u32>,
    frame_count: Option<u32>,
    frame_size: Option<f32>,
    granule: u32,
    size: i32,
    data: Vec<u8>,
    spanning_packet: bool,
    first_packet: bool,
}

impl OpusPacket {
    pub fn new<R: Read>(
        mut filehandle: Option<&mut R>,
        size: i32,
        last_size: i32,
        dont_parse_info: bool,
    ) -> io::Result<Self> {
        let mut packet = OpusPacket {
            size,
            ..Default::default()
        };

        if let Some(fh) = filehandle {
            packet.size = size;
            let mut buf = vec![0u8; size as usize];
            fh.read_exact(&mut buf)?;
            packet.data = buf;
            packet.spanning_packet = size == 255;
            packet.first_packet = last_size != 255;

            if packet.first_packet && !dont_parse_info {
                packet.parse_segment_info()?;
            }
        }

        Ok(packet)
    }

    fn get_frame_count(&self) -> u32 {
        match self.framepacking {
            Some(0) => 1,
            Some(1) | Some(2) => 2,
            Some(3) => {
                if self.data.len() >= 2 {
                    (self.data[1] & 63) as u32
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    fn get_padding(&self) -> u32 {
        if self.framepacking != Some(3) {
            return 0;
        }

        if self.data.len() < 3 {
            return 0;
        }

        let is_padded = (self.data[1] >> 6) & 1;
        if is_padded == 0 {
            return 0;
        }

        let mut total_padding = self.data[2] as u32;
        let mut i = 3;
        let mut padding = self.data[2];

        while padding == 255 && i < self.data.len() {
            padding = self.data[i];
            total_padding = total_padding + padding as u32 - 1;
            i += 1;
        }
        total_padding
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

    fn parse_segment_info(&mut self) -> io::Result<()> {
        if self.data.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Empty data"));
        }

        let byte = self.data[0];
        self.config_value = Some(byte >> 3);
        self.stereo = Some((byte & 4) >> 2);
        self.framepacking = Some(byte & 3);
        self.padding = Some(self.get_padding());
        self.frame_count = Some(self.get_frame_count());
        
        match self.get_frame_size() {
            Ok(frame_size) => {
                self.frame_size = Some(frame_size);
                self.granule = (frame_size * self.frame_count.unwrap_or(0) as f32 * SAMPLE_RATE_KHZ as f32) as u32;
                Ok(())
            }
            Err(e) => Err(io::Error::new(io::ErrorKind::InvalidData, e)),
        }
    }

    pub fn write<W: Write>(&self, mut filehandle: W) -> io::Result<()> {
        if !self.data.is_empty() {
            filehandle.write_all(&self.data)?;
        }
        Ok(())
    }

    pub fn convert_to_framepacking_three(&mut self) {
        if self.framepacking == Some(3) {
            return;
        }

        let mut toc_byte = self.data[0];
        toc_byte |= 0b11;

        let mut frame_count_byte = self.frame_count.unwrap_or(0) as u8;
        if self.framepacking == Some(2) {
            frame_count_byte |= 0b10000000; // vbr
        }

        let mut new_data = Vec::with_capacity(self.data.len() + 1);
        new_data.push(toc_byte);
        new_data.push(frame_count_byte);
        new_data.extend_from_slice(&self.data[1..]);
        
        self.data = new_data;
        self.framepacking = Some(3);
    }

    pub fn set_pad_count(&mut self, count: u32) -> Result<(), &'static str> {
        if self.framepacking != Some(3) {
            return Err("Only code 3 packets can contain padding!");
        }
        if self.padding != Some(0) {
            return Err("Packet already padded. Not supported yet!");
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
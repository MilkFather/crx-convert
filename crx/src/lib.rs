//! CRX Circus Image Format Parser

mod decoder;
use decoder::decode;

use std::{fs, io};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CrxHeader {
    pub inner_x: i16,       // offset 0x04
    pub inner_y: i16,       // offset 0x06
    pub width: u16,         // offset 0x08
    pub height: u16,        // offset 0x0A
    pub version: u16,       // offset 0x0C
    pub flag: u16,          // offset 0x0E
    pub depth: i16,         // offset 0x10
    pub mode: u16,          // offset 0x12
}

#[derive(Debug, Clone)]
pub struct CrxFile {
    header: CrxHeader,
    palette: Vec<[u8; 3]>,
    buffer: Vec<u8>,
}

impl CrxFile {
    /// Build a `CrxFile` object from a buffer.
    pub fn from_buffer(buf: &[u8]) -> io::Result<Self> {
        let mut cursor = io::Cursor::new(buf);
        decode(&mut cursor)
    }

    /// Build a `CrxFile` object from a `std::fs::File` object.
    pub fn from_file(file: &fs::File) -> io::Result<Self> {
        let mut buf = io::BufReader::new(file);
        decode(&mut buf)
    }

    /// Read and build a `CrxFile` object from a specified file name and path.
    pub fn read_from_filename<P>(filename: P) -> io::Result<Self>
    where
        P: AsRef<std::path::Path>
    {
        let file = fs::File::open(filename)?;
        Self::from_file(&file)
    }

}

impl From<CrxFile> for image::DynamicImage {
    fn from(f: CrxFile) -> Self {
        use image::ImageBuffer;

        let bpp = depth_to_bpp(f.header.depth);
        match bpp {
            32 => {
                // BGRA image buffer
                let buf = ImageBuffer::from_fn(f.header.width as u32, f.header.height as u32, |x, y| {
                    let pix_offset = 4_usize * (y as usize * f.header.width as usize + x as usize);
                    let b = f.buffer[pix_offset];
                    let g = f.buffer[pix_offset + 1];
                    let r = f.buffer[pix_offset + 2];
                    let a = f.buffer[pix_offset + 3];
                    image::Rgba([r, g, b, a])
                });
                image::DynamicImage::ImageRgba8(buf)
            },
            24 => {
                // BGR image buffer
                let buf = ImageBuffer::from_fn(f.header.width as u32, f.header.height as u32, |x, y| {
                    let pix_offset = 3_usize * (y as usize * f.header.width as usize + x as usize);
                    let b = f.buffer[pix_offset];
                    let g = f.buffer[pix_offset + 1];
                    let r = f.buffer[pix_offset + 2];
                    image::Rgb([r, g, b])
                });
                image::DynamicImage::ImageRgb8(buf)
            },
            8 => {
                // RGB indexed image buffer
                let buf = ImageBuffer::from_fn(f.header.width as u32, f.header.height as u32, |x, y| {
                    let pix_offset = y as usize * f.header.width as usize + x as usize;
                    let index = f.buffer[pix_offset];
                    image::Rgb(f.palette[index as usize])
                });
                image::DynamicImage::ImageRgb8(buf)
            },
            _ => unreachable!()
        }
    }
}

#[inline(always)]
fn depth_to_bpp(depth: i16) -> u16 {
    if 0 == depth {
        24
    } else if 1 == depth {
        32
    } else {
        8
    }
}

use std::io::{self, Read, Seek, SeekFrom, Cursor};
use std::{fmt, error};

use byteorder::{ReadBytesExt, LittleEndian};

use crate::{CrxFile, CrxHeader, depth_to_bpp};

#[derive(Debug)]
pub enum DecoderError {
    IO(io::Error),
    CrxSignatureInvalid,
    VersionNotSupported(u16),
    InvalidRowDecodeMode(u8),
    InflateFailure(String),
    NoPreviousRow,
}

impl fmt::Display for DecoderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IO(e) => e.fmt(f),
            Self::CrxSignatureInvalid => f.write_str("CRX signature not found"),
            Self::VersionNotSupported(v) => f.write_fmt(format_args!("Unsupported image version: {}", v)),
            Self::InvalidRowDecodeMode(c) => f.write_fmt(format_args!("Invalid row decode mode: {}", c)),
            Self::InflateFailure(s) => s.fmt(f),
            Self::NoPreviousRow => f.write_str("Cannot refer to the previous row"),
        }
    }
}

impl From<DecoderError> for io::Error {
    fn from(e: DecoderError) -> Self {
        match e {
            DecoderError::IO(err) => err,
            _ => Self::new(io::ErrorKind::InvalidData, e.to_string()),
        }
    }
}

impl From<io::Error> for DecoderError {
    fn from(e: io::Error) -> Self {
        Self::IO(e)
    }
}

impl error::Error for DecoderError {}

pub fn decode<R: Read + Seek>(reader: &mut R) -> Result<CrxFile, DecoderError> {
    // Read signature
    let mut sig: [u8; 4] = [0; 4];
    reader.read_exact(&mut sig)?;
    if b"CRXG" != &sig {
        return Err(DecoderError::CrxSignatureInvalid);
    }

    // Read header
    let header = decode_header(reader)?;

    // Read palette
    let palette = if 8 == depth_to_bpp(header.depth) {
        decode_palette(reader, header.depth as usize)?
    } else { Vec::new() };

    // Read some garbage data
    if header.version >= 3 {
        let count = reader.read_i32::<LittleEndian>()?;
        reader.seek(SeekFrom::Current((0x10 * count).into()))?;
    }

    // Read the compressed data
    let data = if (header.flag & 0x10) != 0 {
        // read an int indicating the stream size
        let data_size = reader.read_i32::<LittleEndian>()?;
        let mut buf: Vec<u8> = vec![0; data_size as usize];
        reader.read_exact(&mut buf)?;
        buf
    } else {
        // consume all input
        let mut buf: Vec<u8> = Vec::new();
        reader.read_to_end(&mut buf)?;
        buf
    };

    // Decompress the data
    let mut data = if 1 == header.version {
        unpack_1(&data, &header)?
    } else {
        unpack_2(&data, &header)?
    };

    // Some other operations
    if 32 == depth_to_bpp(header.depth) && header.mode != 1 {
        let alpha_flip: u8 = if 2 == header.mode { 0 } else { 0xFF };
        for h in 0..header.height as usize {
            for w in 0..header.width as usize {
                let offset = (h * header.width as usize + w) * 4; // bpp is 32 as required in `if` condition, byte size is definitely 4
                let alpha = data[offset];
                let b = data[offset + 1];
                let g = data[offset + 2];
                let r = data[offset + 3];
                data[offset] = b;
                data[offset + 1] = g;
                data[offset + 2] = r;
                data[offset + 3] = alpha ^ alpha_flip;
            }
        }
    }

    Ok(CrxFile {
        header,
        palette,
        buffer: data,
    })
}

/// Decodes the header of a CRX file.
fn decode_header<R: Read + Seek>(reader: &mut R) -> Result<CrxHeader, DecoderError> {
    let inner_x = reader.read_i16::<LittleEndian>()?;
    let inner_y = reader.read_i16::<LittleEndian>()?;
    let width = reader.read_u16::<LittleEndian>()?;
    let height = reader.read_u16::<LittleEndian>()?;
    let version = reader.read_u16::<LittleEndian>()?;
    let flag = reader.read_u16::<LittleEndian>()?;
    let depth = reader.read_i16::<LittleEndian>()?;
    let mode = reader.read_u16::<LittleEndian>()?;

    // Verify that the version is supported (1, 2, 3)
    if !(1..=3).contains(&version) {
        return Err(DecoderError::VersionNotSupported(version));
    }
    
    Ok(CrxHeader {
        inner_x, inner_y, width, height, version, flag, depth, mode,
    })
}

/// Decodes the palette of a CRX file.
/// 
/// A palette is present only if the header's `depth` is not 0 or 1.
/// `depth` encodes both the size of the palette, and the depth of each palette color.
fn decode_palette<R: Read + Seek>(reader: &mut R, depth: usize) -> Result<Vec<[u8; 3]>, DecoderError> {
    let color_size = if 0x102 == depth { 4 } else { 3 };
    let colors = if depth > 0x100 { 0x100 } else { depth };
    let mut palette: Vec<[u8; 3]> = Vec::new();

    for _ in 0..colors {
        let r = reader.read_u8()?;
        let mut g = reader.read_u8()?;
        let b = reader.read_u8()?;
        // I don't know why this fourth component exists, even if it is not used.
        if 4 == color_size {
            reader.read_u8()?;
        }
        // Also I don't know why there is no yellow color in the palette.
        if 0xFF == b && 0 == g && 0xFF == r {
            g = 0xFF;
        }
        palette.push([r, g, b]);
    }

    Ok(palette)
}

fn unpack_1(buf: &[u8], header: &CrxHeader) -> Result<Vec<u8>, DecoderError> {
    // The implementation of GARBro seems to be problematic. Tried to fix it.
    let mut window: [u8; 0x10000] = [0; 0x10000];
    let mut flag: i32 = 0;
    let mut win_pos: usize = 0;
    let mut dst: usize = 0;

    let mut buf = Cursor::new(buf);
    let mut output: Vec<u8> = vec![0; (depth_to_bpp(header.depth) as usize / 8) * header.width as usize * header.height as usize];

    while dst < output.len() {
        flag >>= 1;
        if 0 == (flag & 0x100) {
            flag = buf.read_u8()? as i32 | 0xFF00;
        }
        if 0 != (flag & 1) {
            let dat = buf.read_u8()?;
            window[win_pos] = dat;
            win_pos = (win_pos + 1) & 0xFFFF;
            output[dst] = dat;
            dst += 1;
        } else {
            let control: usize = buf.read_u8()? as usize;
            let count: usize;
            let mut offset: usize;

            if control >= 0xC0 {
                offset = ((control & 3) << 8) | (buf.read_u8()? as usize);
                count = 4 + ((control >> 2) & 0xF);
            } else if 0 != (control & 0x80) {
                offset = control & 0x1F;
                count = 2 + ((control >> 5) & 3);
                if 0 == offset {
                    offset = buf.read_u8()? as usize;
                }
            } else if 0x7F == control {
                count = 2 + buf.read_u16::<LittleEndian>()? as usize;
                offset = buf.read_u16::<LittleEndian>()? as usize;
            } else {
                offset = buf.read_u16::<LittleEndian>()? as usize;
                count = control + 4;
            }
            offset = win_pos - offset;
            for _ in 0..count {
                if dst >= output.len() {
                    break;
                }
                offset &= 0xFFFF;
                let dat = window[offset as usize];
                offset += 1;
                window[win_pos] = dat;
                win_pos = (win_pos + 1) & 0xFFFF;
                output[dst] = dat;
                dst += 1;
            }
        }
    }

    Ok(output)
}

fn unpack_2(buf: &[u8], header: &CrxHeader) -> Result<Vec<u8>, DecoderError> {
    let bpp = depth_to_bpp(header.depth);
    let pixel_size = bpp as usize / 8;
    // Number of bytes in a row's data. This applies to both input and output (they have the same value).
    let stride = pixel_size * header.width as usize;

    let mut buf = Cursor::new(inflate::inflate_bytes_zlib(buf).map_err(DecoderError::InflateFailure)?);
    let mut output: Vec<u8> = vec![0; stride * header.height as usize];

    if bpp >= 24 {
        // 24-bit or 32-bit color mode, either in BGR or BGRA
        for y in 0..header.height as usize {
            let ctl = buf.read_u8()?;
            let row_offset = y * stride;
            let prev_row_offset = row_offset.checked_sub(stride);
            match ctl {
                0 => {
                    // First pixel is provided as is, remaining pixels are differences from the previous pixel
                    // Read the first pixel value as is
                    buf.read_exact(&mut output[row_offset..row_offset + pixel_size])?;
                    // Read the remaining pixels (per byte) in the same row
                    for xb in pixel_size..stride {
                        output[row_offset + xb] = buf.read_u8()?.wrapping_add(output[row_offset + xb - pixel_size]);
                    }
                },
                1 => {
                    // Add the difference from the corresponding position from the previous row
                    for xb in 0..stride {
                        output[row_offset + xb] = buf.read_u8()?.wrapping_add(output[prev_row_offset.ok_or(DecoderError::NoPreviousRow)? + xb]);
                    }
                },
                2 => {
                    // First pixel is provided as is, remaining pixels are differences from the the previous row, left-shifting one pixel
                    // Read the first pixel value as is
                    buf.read_exact(&mut output[row_offset..row_offset + pixel_size])?;
                    // Read the remaining pixels (per byte) in the same row
                    for xb in pixel_size..stride {
                        output[row_offset + xb] = buf.read_u8()?.wrapping_add(output[prev_row_offset.ok_or(DecoderError::NoPreviousRow)? + xb - pixel_size]);
                    }
                },
                3 => {
                    // Last pixel is provided as is, pixels before it are differences from the previous row, right-shifting one pixel
                    // Read the pixels
                    for xb in 0..stride - pixel_size {
                        output[row_offset + xb] = buf.read_u8()?.wrapping_add(output[prev_row_offset.ok_or(DecoderError::NoPreviousRow)? + xb + pixel_size]);
                    }
                    // Read the last pixel as is
                    buf.read_exact(&mut output[row_offset + stride - pixel_size..row_offset + stride])?;
                },
                4 => {
                    // Input is organized by pixel component, for each component, same-value compression is used
                    for pix_offset in 0..pixel_size {
                        let mut xb = row_offset + pix_offset;
                        let mut remaining = header.width as isize;

                        // Same-value compression
                        // 1. Read a byte `a`, write to the output.
                        // 2. Read another byte `b`.
                        // 3. If `a == b` then read a third byte `c`, and repeat writing `a` (or `b`) `c` times, go to step 1.
                        // 3. Otherwise set `a` to `b`, go back to step 2.
                        let mut val = buf.read_u8()?; // Row init
                        while remaining > 0 {
                            output[xb] = val;
                            xb += pixel_size;
                            remaining -= 1;
                            if remaining == 0 {
                                break;
                            }
                            let next = buf.read_u8()?;
                            if val == next {
                                let count = buf.read_u8()? as isize;
                                for _ in 0..count {
                                    output[xb] = next;
                                    xb += pixel_size;
                                }
                                remaining -= count;
                                if remaining > 0 {
                                    val = buf.read_u8()?;
                                }
                            } else {
                                val = next;
                            }
                        }
                    }
                }
                other => return Err(DecoderError::InvalidRowDecodeMode(other))
            }
        }
    } else {
        // 8-bit palette color mode, index of color palette (RGB format)
        // Just copy the indices as-is
        buf.read_exact(&mut output)?;
    }

    Ok(output)
}

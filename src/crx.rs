use byteorder::{ReadBytesExt, LittleEndian};
use std::io::{Read, self};

const CRX_SIGNATURE: &[u8; 4] = b"CRXG";

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CrxDecodeError {
    #[error("invalid file signature")]
    CrxSignatureInvalid,
    #[error("unsupported image version `{0}`")]
    VersionNotSupported(u16),
    #[error("invalid row decode mode `{0}`")]
    InvalidRowDecodeMode(u8),
    #[error("cannot refer to previous row")]
    NoPreviousRow,
    #[error("row byte overflow")]
    RowOverflow,
    #[error("bad palette index: palette size is `{0}` but trying to access index `{1}`")]
    BadPaletteIndex(usize, usize),
}

macro_rules! decode_error {
    ($e:expr) => {{ std::io::Error::new(std::io::ErrorKind::InvalidData, $e) }};
}

#[cfg(feature="to_image")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CrxImageConvertError {
    #[error("invalid raw pixel color buffer")]
    InvalidRawBuffer,
    #[error("invalid bpp `{0}`")]
    InvalidBPP(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CrxDataContext {
    width: usize,
    height: usize,
    bpp: usize,
    palette: Vec<[u8; 3]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrxFile {
    inner_x: i16,
    inner_y: i16,
    width: u16,
    height: u16,
    bpp: usize,
    clips: Vec<CrxImageClip>,
    raw_image_buffer: Vec<u8>,
}

impl CrxFile {
    pub fn inner_x(&self) -> i16 {
        self.inner_x
    }

    pub fn inner_y(&self) -> i16 {
        self.inner_y
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    pub fn bpp(&self) -> usize {
        self.bpp
    }

    pub fn clips(&self) -> &[CrxImageClip] {
        &self.clips
    }

    pub fn raw_buffer(&self) -> &[u8] {
        &self.raw_image_buffer
    }

    pub fn read<R: Read>(mut reader: R) -> io::Result<Self> {
        // read signature.
        let sig = {
            let mut sig: [u8; 4] = [0; 4];
            reader.read_exact(&mut sig)?;
            sig
        };
        if sig != *CRX_SIGNATURE {
            return Err(decode_error!(CrxDecodeError::CrxSignatureInvalid));
        }

        // read header.
        let header = CrxHeader::read(reader.by_ref())?;
        let bpp = match header.depth {
            0 => 24,
            1 => 32,
            _ => 8,
        };

        // read palette, iff bpp is 8.
        let palette = if bpp == 8 { Some(Self::read_palette(reader.by_ref(), header.depth as i32)?) } else { None };

        // read clipping information
        let clips = if header.version >= 3 {
            Some(Self::read_clip(reader.by_ref())?)
        } else {
            None
        };

        // read raw compressed data.
        let compressed_data = if (header.flag & 0x10) != 0 {
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

        // prepare decompress context
        let context = CrxDataContext {
            width: header.width as usize,
            height: header.height as usize,
            bpp,
            palette: palette.unwrap_or_default(),
        };

        // decompress (extract) color data.
        let mut color_data = if header.version == 1 {
            Self::unpack_1(&compressed_data, &context)?
        } else {
            Self::unpack_2(&compressed_data, &context)?
        };

        // some final operations I cannot see why.
        if bpp == 32 && header.mode != 1 {
            let alpha_flip: u8 = if 2 == header.mode { 0 } else { 0xFF };
            for h in 0..header.height as usize {
                for w in 0..header.width as usize {
                    let offset = (h * header.width as usize + w) * 4; // bpp is 32 as required in `if` condition, byte size is definitely 4
                    let alpha = color_data[offset];
                    let b = color_data[offset + 1];
                    let g = color_data[offset + 2];
                    let r = color_data[offset + 3];
                    color_data[offset] = b;
                    color_data[offset + 1] = g;
                    color_data[offset + 2] = r;
                    color_data[offset + 3] = alpha ^ alpha_flip;
                }
            }
        }

        // from bgr(a) to rgb(a). only applies when not in indexed mode.
        let pixel_byte = bpp / 8;
        if bpp != 8 {
            for pix in 0..(header.height as usize) * (header.width as usize) {
                color_data.swap(pix * pixel_byte, pix * pixel_byte + 2);
            }
        }

        Ok(Self {
            inner_x: header.inner_x,
            inner_y: header.inner_y,
            width: header.width,
            height: header.height,
            bpp: if bpp == 8 { 24 } else { bpp },
            clips: clips.unwrap_or_default(),
            raw_image_buffer: color_data,
        })
    }

    fn read_palette<R: Read>(mut reader: R, depth: i32) -> io::Result<Vec<[u8; 3]>> {
        let color_size = if depth == 0x102 { 4 } else { 3 };
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

    fn read_clip<R: Read>(mut reader: R) -> io::Result<Vec<CrxImageClip>> {
        let clip_count = reader.read_i32::<LittleEndian>()?;
        let mut clips = Vec::with_capacity(clip_count as usize);
        for _ in 0..clip_count {
            let clip = CrxImageClip::read(reader.by_ref())?;
                clips.push(clip);
        }
        Ok(clips)
    }

    fn unpack_1(buf: &[u8], context: &CrxDataContext) -> io::Result<Vec<u8>> {
        // The implementation of GARBro seems to be problematic. Tried to fix it.
        let mut window: [u8; 0x10000] = [0; 0x10000];
        let mut flag: i32 = 0;
        let mut win_pos: usize = 0;
        let mut dst: usize = 0;

        let mut buf = io::Cursor::new(buf);
        let mut output: Vec<u8> = vec![0; (context.bpp / 8) * context.width * context.height];

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

    fn unpack_2(buf: &[u8], context: &CrxDataContext) -> io::Result<Vec<u8>> {
        use flate2::read::ZlibDecoder;

        let pixel_size = context.bpp / 8;
        let is_palette = pixel_size == 1;
        let pixel_size = if pixel_size == 1 { 3 } else { pixel_size };
        // number of bytes in a row's data. applies to both input and output.
        let stride = pixel_size * context.width;

        let mut reader = ZlibDecoder::new(buf);
        let mut output: Vec<u8> = vec![0; stride * context.height];

        if is_palette {
            // 8-bit palette color mode.
            // palette indices of each pixel are stored here.
            // read palette indices.
            let mut indices: Vec<u8> = vec![0; context.width * context.height];
            reader.read_exact(&mut indices)?;
            // convert palette indices to pixel values.
            for pix in 0..context.width * context.height {
                let index = indices[pix] as usize;
                let color = context.palette.get(index).ok_or(decode_error!(CrxDecodeError::BadPaletteIndex(context.palette.len(), index)))?;
                output[pix * pixel_size] = color[0];
                output[pix * pixel_size + 1] = color[1];
                output[pix * pixel_size + 2] = color[2];
            }
        } else {
            for y in 0..context.height {
                let mode = reader.read_u8()?;
                let row_offset = y * stride;
                let prev_row_offset = row_offset.checked_sub(stride);
                match mode {
                    0 => {
                        // first pixel is provided as is, remaining pixels are encoded as differences from the previous pixel.
                        // read the first pixel value as is.
                        reader.read_exact(&mut output[row_offset..row_offset + pixel_size])?;
                        // read the remaining pixels (per byte) in the same row.
                        for xb in pixel_size..stride {
                            output[row_offset + xb] = reader.read_u8()?.wrapping_add(output[row_offset + xb - pixel_size]);
                        }
                    }
                    1 => {
                        // pixels values are provided as the differences from the corresponding x-position of previous row.
                        let prev_row_offset = prev_row_offset.ok_or(decode_error!(CrxDecodeError::NoPreviousRow))?;
                        for xb in 0..stride {
                            output[row_offset + xb] = reader.read_u8()?.wrapping_add(output[prev_row_offset + xb]);
                        }
                    }
                    2 => {
                        // first pixel is provided as is, remaining pixels are differences from the the previous row, left-shifting one pixel.
                        // get previous row offset.
                        let prev_row_offset = prev_row_offset.ok_or(decode_error!(CrxDecodeError::NoPreviousRow))?;
                        // read the first pixel value as is.
                        reader.read_exact(&mut output[row_offset..row_offset + pixel_size])?;
                        // read the remaining pixels (per byte) in the same row.
                        for xb in pixel_size..stride {
                            output[row_offset + xb] = reader.read_u8()?.wrapping_add(output[prev_row_offset + xb - pixel_size]);
                        }
                    }
                    3 => {
                        // last pixel is provided as is, pixels before it are differences from the previous row, right-shifting one pixel
                        // get previous row offset.
                        let prev_row_offset = prev_row_offset.ok_or(decode_error!(CrxDecodeError::NoPreviousRow))?;
                        // read the pixels
                        for xb in 0..stride - pixel_size {
                            output[row_offset + xb] = reader.read_u8()?.wrapping_add(output[prev_row_offset + xb + pixel_size]);
                        }
                        // read the last pixel as is.
                        reader.read_exact(&mut output[row_offset + stride - pixel_size..row_offset + stride])?;
                    }
                    4 => {
                        // input is organized by pixel component, for each component, same-value compression is used.
                        // same-value compression
                        // 1. read a byte `a`, write to the output.
                        // 2. read another byte `b`.
                        // 3.1. if `a == b` then read a third byte `c`, and repeat writing `a` (or `b`) `c` times, go to step 1.
                        // 3.2. Otherwise set `a` to `b`, go back to step 2.
                        for pix_offset in 0..pixel_size {
                            let mut xb = row_offset + pix_offset;
                            let mut remaining = context.width;
                            let mut val = reader.read_u8()?; // row init
                            while remaining > 0 {
                                output[xb] = val;
                                xb += pixel_size;
                                remaining -= 1;
                                if remaining == 0 {
                                    break;
                                }
                                let next = reader.read_u8()?;
                                if val == next {
                                    let count = reader.read_u8()? as usize;
                                    for _ in 0..count {
                                        output[xb] = next;
                                        xb += pixel_size;
                                    }
                                    remaining = remaining.checked_sub(count).ok_or(decode_error!(CrxDecodeError::RowOverflow))?;
                                    if remaining > 0 {
                                        val = reader.read_u8()?;
                                    }
                                } else {
                                    val = next;
                                }
                            }
                        }
                    }
                    other => return Err(decode_error!(CrxDecodeError::InvalidRowDecodeMode(other)))
                }
            }
        }

        Ok(output)
    }
}

#[cfg(feature="to_image")]
impl TryFrom<CrxFile> for image::DynamicImage {
    type Error = CrxImageConvertError;

    fn try_from(value: CrxFile) -> Result<Self, CrxImageConvertError> {
        match value.bpp {
            24 => {
                let rgb_image = image::ImageBuffer::from_raw(value.width as u32, value.height as u32, value.raw_image_buffer).ok_or(CrxImageConvertError::InvalidRawBuffer)?;
                Ok(image::DynamicImage::ImageRgb8(rgb_image))
            }
            32 => {
                let rgba_image = image::ImageBuffer::from_raw(value.width as u32, value.height as u32, value.raw_image_buffer).ok_or(CrxImageConvertError::InvalidRawBuffer)?;
                Ok(image::DynamicImage::ImageRgba8(rgba_image))
            }
            x => Err(CrxImageConvertError::InvalidBPP(x))
        }
    }
}

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

impl CrxHeader {
    fn read<R: Read>(mut reader: R) -> io::Result<Self> {
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
            return Err(decode_error!(CrxDecodeError::VersionNotSupported(version)));
        }

        Ok(CrxHeader {
            inner_x, inner_y, width, height, version, flag, depth, mode,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrxImageClip {
    pub field_1: i32,
    pub field_2: i16,
    pub field_3: i16,
    pub field_4: i32,
    pub field_5: i16,
    pub field_6: i16,
}

impl CrxImageClip {
    fn read<R: Read>(mut reader: R) -> io::Result<Self> {
        let field_1 = reader.read_i32::<LittleEndian>()?;
        let field_2 = reader.read_i16::<LittleEndian>()?;
        let field_3 = reader.read_i16::<LittleEndian>()?;
        let field_4 = reader.read_i32::<LittleEndian>()?;
        let field_5 = reader.read_i16::<LittleEndian>()?;
        let field_6 = reader.read_i16::<LittleEndian>()?;

        Ok(Self { field_1, field_2, field_3, field_4, field_5, field_6 })
    }
}

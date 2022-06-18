//! CRX Circus Image Format Parser

#[cfg(feature="parse")]
mod parser;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CRXHeader {
    pub inner_x: i16,
    pub inner_y: i16,
    pub width: i16,
    pub height: i16,
    pub version: i16,
    pub flag: i16,
    pub bpp: i16,
    pub unknown: i16,
}

#![cfg(feature="parse")]
//! Parser of CRX Circus Image Format

use nom::IResult;
use nom::bytes::complete::tag;
use nom::combinator::{map, verify};
use nom::number::complete::le_i16;
use nom::sequence::tuple;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CRXHeader {
    pub inner_x: i16,
    pub inner_y: i16,
    pub width: u16,
    pub height: u16,
    pub compression: u16,
    pub flag: u16,
    pub bpp: i16,
    pub mode: u16,
}

fn crx_header(input: &[u8]) -> IResult<&[u8], CRXHeader> {
    verify(
        map(
            tuple((le_i16, le_i16, le_i16, le_i16, le_i16, le_i16, le_i16, le_i16)),
            |(inner_x, inner_y, width, height, version, flag, bpp, unknown)| CRXHeader { inner_x, inner_y, width, height, version, flag, bpp, unknown }
        ),
        |header| (header.version == 2 || header.version == 3) && (header.flag & 0xF) > 1 && (header.bpp == 0 || header.bpp == 1)
    )(input)
}

/*
pub fn parse_crx(input: &[u8]) -> IResult<&[u8], CRXHeader> {
    let (input, header) = map(
        tuple((tag("CRXG"), crx_header)),
        |(_, header)| header
    )(input)?;
    todo!()
}
*/

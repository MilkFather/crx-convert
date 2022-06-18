#![cfg(feature="parse")]
//! Parser of CRX Circus Image Format

use nom::IResult;
use nom::bytes::complete::tag;
use nom::combinator::{map, verify};
use nom::number::complete::le_i16;
use nom::sequence::tuple;

use crate::CRXHeader;

fn crx_header(input: &[u8]) -> IResult<&[u8], CRXHeader> {
    verify(
        map(
            tuple((le_i16, le_i16, le_i16, le_i16, le_i16, le_i16, le_i16, le_i16)),
            |(inner_x, inner_y, width, height, version, flag, bpp, unknown)| CRXHeader { inner_x, inner_y, width, height, version, flag, bpp, unknown }
        ),
        |header| (header.version == 2 || header.version == 3) && (header.flag & 0xF) > 1 && (header.bpp == 0 || header.bpp == 1)
    )(input)
}

pub fn parse_crx(input: &[u8]) -> IResult<&[u8], CRXHeader> {
    let (input, header) = map(
        tuple((tag("CRXG"), crx_header)),
        |(_, header)| header
    )(input)?;
    todo!()
}

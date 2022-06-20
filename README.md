# CRX Circus Image Format
[CIRCUS](http://circus-co.jp) is best known as the developer of *Da Capo* series. They use a properitary format to store their image assets. These files have the file extension `CRX`. Not to be confused with Google Chrome extension format, whose file extension is also `CRX`.

This repository is a [Rust](https://www.rust-lang.org) implementation of [GarBRO](https://github.com/morkt/GARbro)'s [CRX decoder](https://github.com/morkt/GARbro/blob/master/ArcFormats/Circus/ImageCRX.cs). It is cross-examined with [another available decoder implementation](https://github.com/crskycode/CIRCUS_CRX_Tool).

Decoder is placed under `crx/`. Code under `src/` is a tiny tool that uses the decoder and convert CRX files into PNG. Run `cargo build` to build the tool and the decoder. You can convert one file, or all files under a folder (both recursively and not) using the tool.

Reverse engineering is hard. There are certainly files that cannot be decoded. If you have a better understanding of this propertiary image format, please speak out.

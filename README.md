# CRX Circus Image Format
[CIRCUS](http://circus-co.jp) is best known as the developer of *Da Capo* series. They use a properitary format to store their image assets. These files have the file extension `CRX`. Not to be confused with Google Chrome extension format, whose file extension is also `CRX`.

This repository is a [Rust](https://www.rust-lang.org) implementation of [GarBRO](https://github.com/morkt/GARbro)'s [CRX decoder](https://github.com/morkt/GARbro/blob/master/ArcFormats/Circus/ImageCRX.cs). It is cross-examined with [another available decoder implementation](https://github.com/crskycode/CIRCUS_CRX_Tool).

This tool is provided as a library. A sample converter is located at `src/bin/convert.rs`. To build the converter, run
```sh
cargo build --release --bin crx-convert --all-features
```

The simple converter accepts any number of CRX file paths as command line arguments, and convert them into PNG at the same location of the original files.

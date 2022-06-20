use std::{fs, path, ffi};

fn build_arg() -> clap::Command<'static> {
    use clap::{Command, Arg};

    Command::new("CRX CIRCUS Image Format Decoder")
        .about("Decode CRX files in games made by CIRCUS")
        .args(&[
            Arg::new("recursive")
                .short('r')
                .required(false)
                .takes_value(false)
                .help("When decoding a directory, recursively visit sub-directories"),
            Arg::new("uri")
                .value_name("URI")
                .required(true)
                .takes_value(true)
                .multiple_values(true)
                .help("The location of CRX files")
                .long_help("The location of CRX files, you can provide multiple values. If URI is a file, decodes the file. If the URI is a directory, decodes every image in the directory.")
        ])
}

#[inline(always)]
fn print_success<P, Q>(src: P, dst: Q)
where
    P: AsRef<path::Path>,
    Q: AsRef<path::Path>,
{
    println!("Converted \"{}\" -> \"{}\"", src.as_ref().to_string_lossy(), dst.as_ref().to_string_lossy());
}

#[inline(always)]
fn print_fail<P, Q>(src: P, err: Q)
where
    P: AsRef<path::Path>,
    Q: ToString,
{
    println!("   Failed \"{}\": {}", src.as_ref().to_string_lossy(), err.to_string());
}

#[inline(always)]
fn print_skip<P, Q>(src: P, reason: Q)
where
    P: AsRef<path::Path>,
    Q: ToString,
{
    println!("  Skipped \"{}\": {}", src.as_ref().to_string_lossy(), reason.to_string());
}

fn do_one_file<Q>(uri: Q)
where
    Q: AsRef<path::Path>
{
    use crx::CrxFile;
    use image::{DynamicImage, ImageFormat};

    // Determine output path
    let mut output = path::PathBuf::from(uri.as_ref());
    output.set_extension("png");

    let file = CrxFile::read_from_filename(&uri);
    match file {
        Ok(file) => {
            let image: DynamicImage = file.into();
            match image.save_with_format(&output, ImageFormat::Png) {
                Ok(_) => print_success(&uri, &output),
                Err(e) => print_fail(&uri, e),
            }
        },
        Err(crx::DecoderError::CrxSignatureInvalid) => {
            if let Some(ext) = uri.as_ref().extension() {
                if ext.eq_ignore_ascii_case(ffi::OsString::from("CRX")) {
                    print_fail(&uri, crx::DecoderError::CrxSignatureInvalid);
                }
            }
        },
        Err(e) => print_fail(&uri, e),
    }
}

fn main() {
    let arg = build_arg().get_matches();
    let uri: Vec<String> = {
        let uri = arg.get_many("uri");
        if let Some(uri) = uri {
            uri.cloned().collect()
        } else {
            Vec::new()
        }
    };
    for uri in &uri {
        match fs::metadata(uri) {
            Ok(md) => {
                if md.is_file() {
                    do_one_file(uri);
                } else if md.is_dir() {
                    if arg.contains_id("recursive") {
                        for file in walkdir::WalkDir::new(uri).into_iter().filter_map(|f| f.ok()) {
                            if file.metadata().unwrap().is_file() {
                                do_one_file(file.path());
                            }
                        }
                    } else {
                        for file in fs::read_dir(uri).unwrap().filter_map(|f| f.ok()) {
                            if file.metadata().unwrap().is_file() {
                                do_one_file(file.path());
                            }
                        }
                    }
                } else {
                    print_skip(&uri, "neither a file nor a directory");
                }
            },
            Err(e) => print_skip(&uri, e),
        }
    }
}

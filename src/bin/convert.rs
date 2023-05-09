use clap::Parser;
use crx::CrxFile;
use image::{DynamicImage, ImageFormat};
use owo_colors::OwoColorize;
use rayon::prelude::*;
use std::{fs, path::PathBuf, io::{self, BufReader, Read}};

#[derive(Parser)]
struct Arg {
    files: Vec<PathBuf>,
}

fn main() -> io::Result<()> {
    let arg = Arg::parse();

    arg.files.par_iter().for_each(|file| {
        let f = fs::File::open(&file);
        if let Err(e) = f {
            println!("{} \"{}\" read: {}", " Failed".red().bold(), file.to_string_lossy(), e);
            return;
        }
        let mut reader = BufReader::new(f.unwrap());
        let crx_img = CrxFile::read(reader.by_ref());
        if let Err(e) = crx_img {
            println!("{} \"{}\" decode: {}", " Failed".red().bold(), file.to_string_lossy(), e);
            return;
        }
        let crx_img = crx_img.unwrap();
        // println!("clip count: {}", crx_img.clips().len());
        let img = DynamicImage::try_from(crx_img);
        if let Err(e) = img {
            println!("{} \"{}\" convert: {}", " Failed".red().bold(), file.to_string_lossy(), e);
            return;
        }
        let img = img.unwrap();
        // determine output file path
        let output_path = {
            let mut tmp = file.clone();
            tmp.set_extension("png");
            tmp
        };
        // write to file
        let result = img.save_with_format(&output_path, ImageFormat::Png);
        if let Err(e) = result {
            println!("{} \"{}\" save: {}", " Failed".red().bold(), file.to_string_lossy(), e);
            return;
        }
        println!("{} \"{}\" -> \"{}\"", "Success".green().bold(), file.to_string_lossy(), output_path.to_string_lossy());
    });

    Ok(())
}

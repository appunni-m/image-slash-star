//! Diagnostic public-API AVIF frame dump probe.

#![allow(
    unused_crate_dependencies,
    reason = "This diagnostic example only calls the public sequence decoder."
)]

use std::error::Error;
use std::fs;
use std::path::PathBuf;

use image_slash_star::decode_sequence;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let input = fs::read(args.next().ok_or("missing input path")?)?;
    let output = PathBuf::from(args.next().ok_or("missing output directory")?);
    fs::create_dir_all(&output)?;
    let decoded = decode_sequence(&input)?;
    for (index, frame) in decoded.content.frames.iter().enumerate() {
        fs::write(
            output.join(format!("frame-{index}.bin")),
            &frame.image.pixels,
        )?;
    }
    println!("{} frames", decoded.content.frames.len());
    Ok(())
}

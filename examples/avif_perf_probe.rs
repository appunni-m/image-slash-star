#![allow(
    unused_crate_dependencies,
    reason = "This public-API benchmark does not use the crate's implementation-only dependencies."
)]

//! Public-API AVIF sequence timing probe.

use std::error::Error;
use std::fs;
use std::hint::black_box;
use std::time::Instant;

use image_slash_star::decode_sequence;

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args().skip(1);
    let input_path = arguments.next().ok_or("missing AVIF input path")?;
    let repetitions: usize = arguments
        .next()
        .ok_or("missing repetition count")?
        .parse()?;
    let warmup_repetitions: usize = arguments
        .next()
        .map(|value| value.parse())
        .transpose()?
        .unwrap_or(1);
    if arguments.next().is_some() {
        return Err("unexpected argument after warm-up count".into());
    }
    if repetitions == 0 {
        return Err("repetition count must be greater than zero".into());
    }
    let input = fs::read(input_path)?;

    for _ in 0..warmup_repetitions {
        let decoded = decode_sequence(&input)?;
        drop(black_box(decoded));
    }

    for repetition in 0..repetitions {
        let start = Instant::now();
        let decoded = decode_sequence(&input)?;
        drop(black_box(decoded));
        println!("{} {}", repetition, start.elapsed().as_nanos());
    }
    Ok(())
}

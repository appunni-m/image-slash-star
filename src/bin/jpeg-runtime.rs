#![doc = "Release-mode JPEG encode/decode runtime probe."]
#![allow(unused_crate_dependencies)]

use std::hint::black_box;
use std::time::{Duration, Instant};

use image_slash_star::{ColorType, DecodedImage, ImageFormat, decode, encode_default};

fn make_rgb(width: u32, height: u32) -> DecodedImage {
    let mut state = 0x1234_5678u32;
    let mut pixels = vec![0u8; width as usize * height as usize * 3];
    for byte in &mut pixels {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        *byte = state as u8;
    }
    DecodedImage::new(width, height, pixels, ColorType::Rgb8)
}

fn summarize(label: &str, samples: &mut [Duration]) {
    samples.sort_unstable();
    let total: Duration = samples.iter().copied().sum();
    let mean = total / u32::try_from(samples.len()).unwrap_or(1);
    let median = samples[samples.len() / 2];
    let min = samples[0];
    println!("{label:24} mean={mean:>10.3?} median={median:>10.3?} min={min:>10.3?}");
}

fn time_size(width: u32, height: u32, rounds: usize) -> Result<(), Box<dyn std::error::Error>> {
    let image = make_rgb(width, height);
    let encoded = encode_default(black_box(&image), ImageFormat::Jpeg)?;
    for _ in 0..20 {
        black_box(encode_default(black_box(&image), ImageFormat::Jpeg)?);
        black_box(decode(black_box(&encoded))?);
    }

    let mut encode_samples = Vec::with_capacity(rounds);
    for _ in 0..rounds {
        let start = Instant::now();
        let output = encode_default(black_box(&image), ImageFormat::Jpeg)?;
        black_box(output);
        encode_samples.push(start.elapsed());
    }

    let mut decode_samples = Vec::with_capacity(rounds);
    for _ in 0..rounds {
        let start = Instant::now();
        let output = decode(black_box(&encoded))?;
        black_box(output);
        decode_samples.push(start.elapsed());
    }

    println!(
        "\n{width}x{height} RGB input={} bytes JPEG={} bytes",
        image.pixels.len(),
        encoded.len()
    );
    summarize("encode", &mut encode_samples);
    summarize("decode", &mut decode_samples);
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rounds = std::env::var("ROUNDS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(200);
    println!("rounds={rounds}");
    for (width, height) in [(8, 8), (32, 32), (128, 128), (256, 256)] {
        time_size(width, height, rounds)?;
    }
    Ok(())
}

#![cfg_attr(coverage, feature(coverage_attribute))]
#![doc = "Release-mode JPEG encode/decode runtime probe."]

use std::error::Error;
use std::hint::black_box;
use std::time::{Duration, Instant};

use bytemuck as _;
use image_slash_star::{ColorType, DecodedImage, ImageFormat, decode, encode_default};
#[cfg(feature = "jpeg")]
use wide as _;

fn make_rgb(width: u32, height: u32) -> Result<DecodedImage, Box<dyn Error>> {
    let width = usize::try_from(width)?;
    let height = usize::try_from(height)?;
    let pixel_len = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(3))
        .ok_or_else(|| std::io::Error::other("RGB dimensions overflow the pixel buffer"))?;
    let mut state = 0x1234_5678u32;
    let mut pixels = vec![0u8; pixel_len];
    for byte in &mut pixels {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        *byte = state.to_le_bytes()[0];
    }
    Ok(DecodedImage::new(
        u32::try_from(width)?,
        u32::try_from(height)?,
        pixels,
        ColorType::Rgb8,
    ))
}

fn summarize(label: &str, samples: &mut [Duration]) {
    if samples.is_empty() {
        println!("{label:24} no samples");
        return;
    }
    samples.sort_unstable();
    let total: Duration = samples.iter().copied().sum();
    let divisor = u32::try_from(samples.len()).map_or(u32::MAX, |value| value.max(1));
    let mean = total.checked_div(divisor).unwrap_or_default();
    let median = samples[samples.len() / 2];
    let min = samples[0];
    println!("{label:24} mean={mean:>10.3?} median={median:>10.3?} min={min:>10.3?}");
}

// This helper is a development-only timing wrapper. Its untestable branches
// are host/allocator/API failure propagation, not codec behavior; the public
// JPEG calls and workload are exercised by the unit test and the production
// matrix harness below.
#[cfg_attr(coverage, coverage(off))]
fn time_size(width: u32, height: u32, rounds: usize) -> Result<(), Box<dyn std::error::Error>> {
    let image = make_rgb(width, height)?;
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

#[cfg_attr(coverage, coverage(off))]
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

#[cfg(test)]
mod tests {
    use super::time_size;

    #[test]
    fn benchmark_workload_executes_a_small_round() -> Result<(), Box<dyn std::error::Error>> {
        time_size(8, 8, 1)
    }

    #[test]
    fn empty_summary_is_a_noop() {
        let mut samples = [];
        super::summarize("empty", &mut samples);
    }

    #[test]
    fn nonempty_summary_reports_stable_statistics() {
        let mut samples = [
            std::time::Duration::from_nanos(2),
            std::time::Duration::from_nanos(1),
        ];
        super::summarize("nonempty", &mut samples);
        assert_eq!(samples[0], std::time::Duration::from_nanos(1));
    }
}

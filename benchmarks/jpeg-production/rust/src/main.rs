use std::env;
use std::error::Error;
use std::fmt::Display;
use std::hint::black_box;
use std::io::{Error as IoError, ErrorKind};
use std::str::FromStr;
use std::time::Instant;

use image_slash_star::{
    CancellationToken, ColorType, DecodedImage, EncodeOptions, ImageFormat, JpegEncodeOptions,
    JpegSubsampling, decode, encode, encode_with_token,
};

const WARMUP: usize = 100;

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn invalid_input(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(IoError::new(ErrorKind::InvalidInput, message.into()))
}

fn argument<'a>(args: &'a [String], index: usize, name: &str) -> Result<&'a str, Box<dyn Error>> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| invalid_input(format!("missing {name}")))
}

fn parse_argument<T>(args: &[String], index: usize, name: &str) -> Result<T, Box<dyn Error>>
where
    T: FromStr,
    T::Err: Display,
{
    argument(args, index, name)?
        .parse::<T>()
        .map_err(|error| invalid_input(format!("invalid {name}: {error}")))
}

fn generated_pixels(
    width: usize,
    height: usize,
    channels: usize,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut state = 0x1234_5678u32;
    let pixel_count = width
        .checked_mul(height)
        .and_then(|count| count.checked_mul(channels))
        .ok_or_else(|| invalid_input("benchmark image dimensions overflow"))?;
    let mut pixels = vec![0u8; pixel_count];
    for byte in &mut pixels {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        *byte = state.to_le_bytes()[0];
    }
    Ok(pixels)
}

fn quantile(sorted: &[u128], numerator: usize, denominator: usize) -> Result<u128, Box<dyn Error>> {
    if sorted.is_empty() || denominator == 0 {
        return Err(invalid_input("cannot calculate a quantile without samples"));
    }
    let adjustment = denominator
        .checked_sub(1)
        .ok_or_else(|| invalid_input("quantile denominator is invalid"))?;
    let rounded = sorted
        .len()
        .checked_mul(numerator)
        .ok_or_else(|| invalid_input("quantile calculation overflowed"))?
        .checked_add(adjustment)
        .ok_or_else(|| invalid_input("quantile calculation overflowed"))?;
    let index = rounded
        .checked_div(denominator)
        .ok_or_else(|| invalid_input("quantile denominator is invalid"))?
        .saturating_sub(1)
        .min(sorted.len().saturating_sub(1));
    sorted
        .get(index)
        .copied()
        .ok_or_else(|| invalid_input("quantile index is outside the sample set"))
}

fn report(
    operation: &str,
    iterations: usize,
    total_ns: u128,
    mut samples: Vec<u128>,
) -> Result<(), Box<dyn Error>> {
    if iterations == 0 {
        return Err(invalid_input("iterations must be greater than zero"));
    }
    samples.sort_unstable();
    println!("operation={operation}");
    println!("implementation=image-slash-star");
    println!("boundary=public-api-fresh-operation-owned-output");
    println!("iterations={iterations}");
    println!("warmup={WARMUP}");
    let divisor = u128::try_from(iterations)
        .map_err(|_| invalid_input("iteration count is not representable"))?;
    let average = total_ns
        .checked_div(divisor)
        .ok_or_else(|| invalid_input("average calculation failed"))?;
    println!("avg_ns={average}");
    println!("median_ns={}", quantile(&samples, 1, 2)?);
    println!("p95_ns={}", quantile(&samples, 95, 100)?);
    println!(
        "min_ns={}",
        samples
            .first()
            .copied()
            .ok_or_else(|| { invalid_input("benchmark produced no samples") })?
    );
    Ok(())
}

fn options(
    quality: u8,
    subsampling: &str,
    progressive: bool,
    optimize: bool,
    restart: u32,
) -> Result<EncodeOptions, Box<dyn Error>> {
    let mut options = JpegEncodeOptions::default();
    options.quality = Some(quality);
    options.progressive = Some(progressive);
    options.optimize = Some(optimize);
    options.subsampling = Some(match subsampling {
        "444" => JpegSubsampling::Cs444,
        "422" => JpegSubsampling::Cs422,
        "420" => JpegSubsampling::Cs420,
        _ => return Err(invalid_input("subsampling must be 444, 422, or 420")),
    });
    options.restart_interval = Some(restart);
    Ok(options.into())
}

fn parse_bool(value: &str) -> Result<bool, Box<dyn Error>> {
    match value {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err(invalid_input("boolean must be 0 or 1")),
    }
}

fn mode(value: &str) -> Result<(ColorType, usize), Box<dyn Error>> {
    match value {
        "rgb" => Ok((ColorType::Rgb8, 3)),
        "gray" => Ok((ColorType::L8, 1)),
        "cmyk" => Ok((ColorType::Cmyk8, 4)),
        _ => Err(invalid_input("mode must be rgb, gray, or cmyk")),
    }
}

fn emit(args: &[String], with_token: bool) -> Result<(), Box<dyn Error>> {
    if args.len() != 11 {
        return Err(invalid_input(
            "emit WIDTH HEIGHT rgb|gray|cmyk QUALITY SUBSAMPLING PROGRESSIVE OPTIMIZE RESTART_ROWS OUTPUT",
        ));
    }
    let width = parse_argument::<usize>(args, 2, "width")?;
    let height = parse_argument::<usize>(args, 3, "height")?;
    let (mode, channels) = mode(argument(args, 4, "mode")?)?;
    let quality = parse_argument::<u8>(args, 5, "quality")?;
    let options = options(
        quality,
        argument(args, 6, "subsampling")?,
        parse_bool(argument(args, 7, "progressive")?)?,
        parse_bool(argument(args, 8, "optimize")?)?,
        parse_argument::<u32>(args, 9, "restart rows")?,
    )?;
    let width_u32 = u32::try_from(width)
        .map_err(|_| invalid_input("width does not fit the public image dimensions"))?;
    let height_u32 = u32::try_from(height)
        .map_err(|_| invalid_input("height does not fit the public image dimensions"))?;
    let image = DecodedImage::new(
        width_u32,
        height_u32,
        generated_pixels(width, height, channels)?,
        mode,
    );
    let output = if with_token {
        encode_with_token(
            &image,
            ImageFormat::Jpeg,
            &options,
            &CancellationToken::new(),
        )?
    } else {
        encode(&image, ImageFormat::Jpeg, &options)?
    };
    std::fs::write(argument(args, 10, "output path")?, &output)?;
    println!(
        "output_bytes={}\noutput_fnv1a={:016x}",
        output.len(),
        fnv1a(&output)
    );
    Ok(())
}

fn benchmark_encode(args: &[String]) -> Result<(), Box<dyn Error>> {
    if args.len() != 11 {
        return Err(invalid_input(
            "encode WIDTH HEIGHT rgb|gray|cmyk QUALITY SUBSAMPLING PROGRESSIVE OPTIMIZE RESTART_ROWS ITERATIONS",
        ));
    }
    let width = parse_argument::<usize>(args, 2, "width")?;
    let height = parse_argument::<usize>(args, 3, "height")?;
    let mode_name = argument(args, 4, "mode")?;
    let (mode, channels) = mode(mode_name)?;
    let quality = parse_argument::<u8>(args, 5, "quality")?;
    let subsampling = argument(args, 6, "subsampling")?;
    let progressive = parse_bool(argument(args, 7, "progressive")?)?;
    let optimize = parse_bool(argument(args, 8, "optimize")?)?;
    let restart = parse_argument::<u32>(args, 9, "restart rows")?;
    let iterations = parse_argument::<usize>(args, 10, "iterations")?;
    let pixels = generated_pixels(width, height, channels)?;
    let input_hash = fnv1a(&pixels);
    let width_u32 = u32::try_from(width)
        .map_err(|_| invalid_input("width does not fit the public image dimensions"))?;
    let height_u32 = u32::try_from(height)
        .map_err(|_| invalid_input("height does not fit the public image dimensions"))?;
    let image = DecodedImage::new(width_u32, height_u32, pixels, mode);
    let options = options(quality, subsampling, progressive, optimize, restart)?;
    let invoke = || encode(black_box(&image), ImageFormat::Jpeg, black_box(&options));
    for _ in 0..WARMUP {
        black_box(invoke()?);
    }
    let probe = invoke()?;
    println!(
        "width={width}\nheight={height}\nmode={mode_name}\nquality={quality}\nsubsampling={subsampling}\nprogressive={}\noptimize={}\nrestart_rows={restart}",
        u8::from(progressive),
        u8::from(optimize)
    );
    println!(
        "input_fnv1a={input_hash:016x}\noutput_bytes={}\noutput_fnv1a={:016x}",
        probe.len(),
        fnv1a(&probe)
    );
    let mut samples = Vec::with_capacity(iterations);
    let total = Instant::now();
    for _ in 0..iterations {
        let start = Instant::now();
        let output = invoke()?;
        black_box(&output);
        drop(output);
        samples.push(start.elapsed().as_nanos());
    }
    report("encode", iterations, total.elapsed().as_nanos(), samples)
}

fn benchmark_decode(args: &[String]) -> Result<(), Box<dyn Error>> {
    if args.len() != 4 {
        return Err(invalid_input("decode JPEG ITERATIONS"));
    }
    let jpeg = std::fs::read(argument(args, 2, "JPEG path")?)?;
    let iterations = parse_argument::<usize>(args, 3, "iterations")?;
    let invoke = || decode(black_box(&jpeg));
    for _ in 0..WARMUP {
        black_box(invoke()?);
    }
    let probe = invoke()?;
    println!(
        "input_bytes={}\ninput_fnv1a={:016x}",
        jpeg.len(),
        fnv1a(&jpeg)
    );
    println!(
        "width={}\nheight={}\nmode={:?}\noutput_bytes={}\noutput_fnv1a={:016x}",
        probe.content.width,
        probe.content.height,
        probe.content.mode,
        probe.content.pixels.len(),
        fnv1a(&probe.content.pixels)
    );
    let mut samples = Vec::with_capacity(iterations);
    let total = Instant::now();
    for _ in 0..iterations {
        let start = Instant::now();
        let output = invoke()?;
        black_box(&output);
        drop(output);
        samples.push(start.elapsed().as_nanos());
    }
    report("decode", iterations, total.elapsed().as_nanos(), samples)
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = env::args().collect::<Vec<_>>();
    match argument(&args, 1, "operation")? {
        "emit" => emit(&args, false),
        "emit-token" => emit(&args, true),
        "encode" => benchmark_encode(&args),
        "decode" => benchmark_decode(&args),
        _ => Err(invalid_input(
            "operation must be emit, emit-token, encode, or decode",
        )),
    }
}

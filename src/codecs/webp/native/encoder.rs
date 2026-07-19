//! Encoding of WebP images.
use std::io::{self, Write};

mod backward_refs;
pub(super) mod cross_color;
pub(super) mod predictor;

/// Color type of the image.
///
/// Note that the WebP format doesn't have a concept of color type. All images are encoded as RGBA
/// and some decoders may treat them as such. This enum is used to indicate the color type of the
/// input data provided to the encoder, which can help improve compression ratio.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ColorType {
    /// Opaque image with a single luminance byte per pixel.
    L8,
    /// Image with a luminance and alpha byte per pixel.
    La8,
    /// Opaque image with a red, green, and blue byte per pixel.
    Rgb8,
    /// Image with a red, green, blue, and alpha byte per pixel.
    Rgba8,
}

/// Error encountered while encoding lossless WebP data.
#[derive(Debug)]
#[non_exhaustive]
pub enum EncodingError {
    IoError(io::Error),
    InvalidDimensions,
}

impl From<io::Error> for EncodingError {
    fn from(error: io::Error) -> Self {
        Self::IoError(error)
    }
}

impl std::fmt::Display for EncodingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IoError(error) => write!(formatter, "WebP I/O error: {error}"),
            Self::InvalidDimensions => formatter.write_str("invalid WebP dimensions"),
        }
    }
}

impl std::error::Error for EncodingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::IoError(error) => Some(error),
            Self::InvalidDimensions => None,
        }
    }
}

struct BitWriter<W> {
    writer: W,
    buffer: u64,
    nbits: u8,
}

impl<W: Write> BitWriter<W> {
    fn write_bits(&mut self, bits: u64, nbits: u8) -> io::Result<()> {
        debug_assert!(nbits <= 64);

        self.buffer |= bits << self.nbits;
        self.nbits += nbits;

        if self.nbits >= 64 {
            self.writer.write_all(&self.buffer.to_le_bytes())?;
            self.nbits -= 64;
            self.buffer = bits.checked_shr(u32::from(nbits - self.nbits)).unwrap_or(0);
        }
        debug_assert!(self.nbits < 64);
        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.nbits % 8 != 0 {
            self.write_bits(0, 8 - self.nbits % 8)?;
        }
        if self.nbits > 0 {
            self.writer
                .write_all(&self.buffer.to_le_bytes()[..self.nbits as usize / 8])
                .unwrap();
            self.buffer = 0;
            self.nbits = 0;
        }
        Ok(())
    }
}

fn write_single_entry_huffman_tree<W: Write>(w: &mut BitWriter<W>, symbol: u8) -> io::Result<()> {
    w.write_bits(1, 2)?;
    if symbol <= 1 {
        w.write_bits(0, 1)?;
        w.write_bits(u64::from(symbol), 1)?;
    } else {
        w.write_bits(1, 1)?;
        w.write_bits(u64::from(symbol), 8)?;
    }
    Ok(())
}

fn build_huffman_tree(
    frequencies: &[u32],
    lengths: &mut [u8],
    codes: &mut [u16],
    length_limit: u8,
) -> bool {
    assert_eq!(frequencies.len(), lengths.len());
    assert_eq!(frequencies.len(), codes.len());

    if frequencies.iter().filter(|&&f| f > 0).count() <= 1 {
        lengths.fill(0);
        codes.fill(0);
        return false;
    }

    #[derive(Clone)]
    enum Node {
        Leaf(usize),
        Branch(Box<Node>, Box<Node>),
    }
    #[derive(Clone)]
    struct WeightedNode {
        count: u32,
        sort_value: usize,
        node: Node,
    }

    let mut count_min = 1_u32;
    loop {
        let mut nodes = frequencies
            .iter()
            .enumerate()
            .filter(|&(_, &frequency)| frequency != 0)
            .map(|(value, &frequency)| WeightedNode {
                count: frequency.max(count_min),
                sort_value: value,
                node: Node::Leaf(value),
            })
            .collect::<Vec<_>>();
        nodes.sort_by(|left, right| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left.sort_value.cmp(&right.sort_value))
        });
        while nodes.len() > 1 {
            let left = nodes.pop().unwrap();
            let right = nodes.pop().unwrap();
            let count = left.count + right.count;
            let position = nodes
                .iter()
                .position(|node| node.count <= count)
                .unwrap_or(nodes.len());
            nodes.insert(
                position,
                WeightedNode {
                    count,
                    sort_value: usize::MAX,
                    node: Node::Branch(Box::new(left.node), Box::new(right.node)),
                },
            );
        }

        lengths.fill(0);
        let mut stack = vec![(&nodes[0].node, 0_u8)];
        while let Some((node, depth)) = stack.pop() {
            match node {
                Node::Leaf(value) => lengths[*value] = depth,
                Node::Branch(left, right) => {
                    stack.push((right, depth + 1));
                    stack.push((left, depth + 1));
                }
            }
        }
        if lengths.iter().copied().max().unwrap_or(0) <= length_limit {
            break;
        }
        count_min *= 2;
    }

    // Assign codes
    codes.fill(0);
    let mut code = 0u32;
    for len in 1..=length_limit {
        for (i, &length) in lengths.iter().enumerate() {
            if length == len {
                codes[i] = (code as u16).reverse_bits() >> (16 - len);
                code += 1;
            }
        }
        code <<= 1;
    }
    true
}

fn write_huffman_tree<W: Write>(
    w: &mut BitWriter<W>,
    frequencies: &[u32],
    lengths: &mut [u8],
    codes: &mut [u16],
) -> io::Result<()> {
    let symbols = frequencies
        .iter()
        .enumerate()
        .filter_map(|(symbol, &frequency)| (frequency != 0).then_some(symbol))
        .take(3)
        .collect::<Vec<_>>();
    if symbols.len() <= 2 && symbols.iter().all(|&symbol| symbol < 256) {
        let first = symbols.first().copied().unwrap_or(0);
        w.write_bits(1, 1)?;
        w.write_bits(u64::from(symbols.len() == 2), 1)?;
        if first <= 1 {
            w.write_bits(0, 1)?;
            w.write_bits(first as u64, 1)?;
        } else {
            w.write_bits(1, 1)?;
            w.write_bits(first as u64, 8)?;
        }
        if symbols.len() == 2 {
            w.write_bits(symbols[1] as u64, 8)?;
            lengths.fill(0);
            codes.fill(0);
            lengths[symbols[0]] = 1;
            lengths[symbols[1]] = 1;
            codes[symbols[1]] = 1;
        }
        return Ok(());
    }
    if !build_huffman_tree(frequencies, lengths, codes, 15) {
        let symbol = frequencies
            .iter()
            .position(|&frequency| frequency > 0)
            .unwrap_or(0);
        return write_single_entry_huffman_tree(w, symbol as u8);
    }
    let mut code_length_lengths = [0u8; 16];
    let mut code_length_codes = [0u16; 16];
    let mut code_length_frequencies = [0u32; 16];
    for &length in lengths.iter() {
        code_length_frequencies[length as usize] += 1;
    }
    let single_code_length_length = !build_huffman_tree(
        &code_length_frequencies,
        &mut code_length_lengths,
        &mut code_length_codes,
        7,
    );
    const CODE_LENGTH_ORDER: [usize; 19] = [
        17, 18, 0, 1, 2, 3, 4, 5, 16, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
    ];

    // Write the huffman tree
    w.write_bits(0, 1)?; // normal huffman tree
    w.write_bits(19 - 4, 4)?; // num_code_lengths - 4

    for i in CODE_LENGTH_ORDER {
        if i > 15 || code_length_frequencies[i] == 0 {
            w.write_bits(0, 3)?;
        } else if single_code_length_length {
            w.write_bits(1, 3)?;
        } else {
            w.write_bits(u64::from(code_length_lengths[i]), 3)?;
        }
    }

    match lengths.len() {
        256 => {
            w.write_bits(1, 1)?; // max_symbol is stored
            w.write_bits(3, 3)?; // max_symbol_nbits / 2 - 2
            w.write_bits(254, 8)?; // max_symbol - 2
        }
        280 => w.write_bits(0, 1)?,
        _ => w.write_bits(0, 1)?,
    }

    // Write the huffman codes
    if !single_code_length_length {
        for &len in lengths.iter() {
            w.write_bits(
                u64::from(code_length_codes[len as usize]),
                code_length_lengths[len as usize],
            )?;
        }
    }

    Ok(())
}

const fn length_to_symbol(len: usize) -> (usize, u8) {
    if len <= 4 {
        return (len - 1, 0);
    }
    let len = len - 1;
    let highest_bit = len.ilog2() as usize;
    let second_highest_bit = (len >> (highest_bit - 1)) & 1;
    let extra_bits = highest_bit - 1;
    let symbol = 2 * highest_bit + second_highest_bit;
    (symbol, extra_bits as u8)
}

#[inline(always)]
fn count_run(
    pixel: u32,
    it: &mut std::iter::Peekable<std::slice::Iter<'_, u32>>,
    frequencies1: &mut [u32; 280],
) {
    let mut run_length = 0;
    while run_length < 4096 && it.peek().is_some_and(|&&next| next == pixel) {
        run_length += 1;
        it.next();
    }
    if run_length > 0 {
        if run_length <= 4 {
            let symbol = 256 + run_length - 1;
            frequencies1[symbol] += 1;
        } else {
            let (symbol, _extra_bits) = length_to_symbol(run_length);
            frequencies1[256 + symbol] += 1;
        }
    }
}

#[inline(always)]
fn write_run<W: Write>(
    w: &mut BitWriter<W>,
    pixel: u32,
    it: &mut std::iter::Peekable<std::slice::Iter<'_, u32>>,
    codes1: &[u16; 280],
    lengths1: &[u8; 280],
) -> io::Result<()> {
    let mut run_length = 0;
    while run_length < 4096 && it.peek().is_some_and(|&&next| next == pixel) {
        run_length += 1;
        it.next();
    }
    if run_length > 0 {
        if run_length <= 4 {
            let symbol = 256 + run_length - 1;
            w.write_bits(u64::from(codes1[symbol]), lengths1[symbol])?;
        } else {
            let (symbol, extra_bits) = length_to_symbol(run_length);
            w.write_bits(u64::from(codes1[256 + symbol]), lengths1[256 + symbol])?;
            w.write_bits(
                (run_length as u64 - 1) & ((1 << extra_bits) - 1),
                extra_bits,
            )?;
        }
    }
    Ok(())
}

#[inline]
fn channels(pixel: u32) -> [usize; 4] {
    [
        ((pixel >> 16) & 0xff) as usize,
        ((pixel >> 8) & 0xff) as usize,
        (pixel & 0xff) as usize,
        (pixel >> 24) as usize,
    ]
}

fn write_image_stream<W: Write>(
    w: &mut BitWriter<W>,
    pixels: &[u32],
    width: usize,
    write_meta_huffman_bit: bool,
) -> io::Result<()> {
    let (tokens, cache_bits) = backward_refs::select(pixels, width, write_meta_huffman_bit);
    w.write_bits(u64::from(cache_bits != 0), 1)?;
    if cache_bits != 0 {
        w.write_bits(u64::from(cache_bits), 4)?;
    }
    if write_meta_huffman_bit {
        w.write_bits(0, 1)?; // one global Huffman group
    }

    let mut frequencies0 = [0_u32; 256];
    let cache_size = if cache_bits == 0 { 0 } else { 1 << cache_bits };
    let mut frequencies1 = vec![0_u32; 280 + cache_size];
    let mut frequencies2 = [0_u32; 256];
    let mut frequencies3 = [0_u32; 256];
    let mut frequencies4 = [0_u32; 40];
    for &token in &tokens {
        match token {
            backward_refs::Token::Literal(pixel) => {
                let [red, green, blue, alpha] = channels(pixel);
                frequencies0[red] += 1;
                frequencies1[green] += 1;
                frequencies2[blue] += 1;
                frequencies3[alpha] += 1;
            }
            backward_refs::Token::Copy { distance, length } => {
                let (symbol, _) = length_to_symbol(length);
                frequencies1[256 + symbol] += 1;
                let distance = backward_refs::plane_code(width, distance);
                let (symbol, _) = length_to_symbol(distance);
                frequencies4[symbol] += 1;
            }
            backward_refs::Token::Cache(index) => frequencies1[280 + index] += 1,
        }
    }

    let mut lengths0 = [0_u8; 256];
    let mut lengths1 = vec![0_u8; frequencies1.len()];
    let mut lengths2 = [0_u8; 256];
    let mut lengths3 = [0_u8; 256];
    let mut lengths4 = [0_u8; 40];
    let mut codes0 = [0_u16; 256];
    let mut codes1 = vec![0_u16; frequencies1.len()];
    let mut codes2 = [0_u16; 256];
    let mut codes3 = [0_u16; 256];
    let mut codes4 = [0_u16; 40];
    write_huffman_tree(w, &frequencies1, &mut lengths1, &mut codes1)?;
    write_huffman_tree(w, &frequencies0, &mut lengths0, &mut codes0)?;
    write_huffman_tree(w, &frequencies2, &mut lengths2, &mut codes2)?;
    write_huffman_tree(w, &frequencies3, &mut lengths3, &mut codes3)?;
    write_huffman_tree(w, &frequencies4, &mut lengths4, &mut codes4)?;

    for token in tokens {
        match token {
            backward_refs::Token::Literal(pixel) => {
                let [red, green, blue, alpha] = channels(pixel);
                let green_length = lengths1[green];
                let red_length = lengths0[red];
                let blue_length = lengths2[blue];
                let alpha_length = lengths3[alpha];
                let code = u64::from(codes1[green])
                    | (u64::from(codes0[red]) << green_length)
                    | (u64::from(codes2[blue]) << (green_length + red_length))
                    | (u64::from(codes3[alpha]) << (green_length + red_length + blue_length));
                w.write_bits(code, green_length + red_length + blue_length + alpha_length)?;
            }
            backward_refs::Token::Copy { distance, length } => {
                let (symbol, extra_bits) = length_to_symbol(length);
                let symbol = 256 + symbol;
                w.write_bits(u64::from(codes1[symbol]), lengths1[symbol])?;
                w.write_bits(((length - 1) & ((1 << extra_bits) - 1)) as u64, extra_bits)?;
                let distance = backward_refs::plane_code(width, distance);
                let (symbol, extra_bits) = length_to_symbol(distance);
                w.write_bits(u64::from(codes4[symbol]), lengths4[symbol])?;
                w.write_bits(
                    ((distance - 1) & ((1 << extra_bits) - 1)) as u64,
                    extra_bits,
                )?;
            }
            backward_refs::Token::Cache(index) => {
                let symbol = 280 + index;
                w.write_bits(u64::from(codes1[symbol]), lengths1[symbol])?;
            }
        }
    }
    Ok(())
}

/// Allows fine-tuning some encoder parameters.
///
/// Pass to [`WebPEncoder::set_params()`].
#[non_exhaustive]
#[derive(Clone, Debug)]
pub struct EncoderParams {
    /// Use a predictor transform. Enabled by default.
    pub use_predictor_transform: bool,
}

impl Default for EncoderParams {
    fn default() -> Self {
        Self {
            use_predictor_transform: true,
        }
    }
}

/// Encode image data with the indicated color type.
///
/// # Panics
///
/// Panics if the image data is not of the indicated dimensions.
fn encode_frame<W: Write>(
    writer: W,
    data: &[u8],
    width: u32,
    height: u32,
    color: ColorType,
    params: EncoderParams,
) -> Result<(), EncodingError> {
    let w = &mut BitWriter {
        writer,
        buffer: 0,
        nbits: 0,
    };

    let (is_alpha, bytes_per_pixel) = match color {
        ColorType::L8 => (false, 1),
        ColorType::La8 => (true, 2),
        ColorType::Rgb8 => (false, 3),
        ColorType::Rgba8 => (true, 4),
    };

    assert_eq!(
        (u64::from(width) * u64::from(height)).saturating_mul(bytes_per_pixel),
        data.len() as u64
    );

    if width == 0 || width > 16384 || height == 0 || height > 16384 {
        return Err(EncodingError::InvalidDimensions);
    }

    w.write_bits(0x2f, 8)?; // signature
    w.write_bits(u64::from(width) - 1, 14)?;
    w.write_bits(u64::from(height) - 1, 14)?;

    w.write_bits(u64::from(is_alpha), 1)?; // alpha used
    w.write_bits(0x0, 3)?; // version

    let mut pixels: Vec<u32> = match color {
        ColorType::L8 => data
            .iter()
            .map(|&value| {
                0xff00_0000 | (u32::from(value) << 16) | (u32::from(value) << 8) | u32::from(value)
            })
            .collect(),
        ColorType::La8 => data
            .chunks_exact(2)
            .map(|pixel| {
                (u32::from(pixel[1]) << 24)
                    | (u32::from(pixel[0]) << 16)
                    | (u32::from(pixel[0]) << 8)
                    | u32::from(pixel[0])
            })
            .collect(),
        ColorType::Rgb8 => data
            .chunks_exact(3)
            .map(|pixel| {
                0xff00_0000
                    | (u32::from(pixel[0]) << 16)
                    | (u32::from(pixel[1]) << 8)
                    | u32::from(pixel[2])
            })
            .collect(),
        ColorType::Rgba8 => data
            .chunks_exact(4)
            .map(|pixel| {
                (u32::from(pixel[3]) << 24)
                    | (u32::from(pixel[0]) << 16)
                    | (u32::from(pixel[1]) << 8)
                    | u32::from(pixel[2])
            })
            .collect(),
    };

    if params.use_predictor_transform {
        let (predictor_map, predictor_bits) =
            predictor::select_and_apply(&mut pixels, width as usize, height as usize, 3);
        w.write_bits(1, 1)?;
        w.write_bits(0, 2)?;
        w.write_bits(u64::from(predictor_bits - 2), 3)?;
        let predictor_width = (width as usize + (1 << predictor_bits) - 1) >> predictor_bits;
        write_image_stream(w, &predictor_map, predictor_width, false)?;

        let (color_map, color_bits) =
            cross_color::select_and_apply(&mut pixels, width as usize, height as usize, 3, 80);
        w.write_bits(1, 1)?;
        w.write_bits(1, 2)?;
        w.write_bits(u64::from(color_bits - 2), 3)?;
        let color_width = (width as usize + (1 << color_bits) - 1) >> color_bits;
        write_image_stream(w, &color_map, color_width, false)?;
    }

    w.write_bits(0, 1)?; // transforms done
    write_image_stream(w, &pixels, width as usize, true)?;

    w.flush()?;
    Ok(())
}

const fn chunk_size(inner_bytes: usize) -> u32 {
    if inner_bytes % 2 == 1 {
        (inner_bytes + 1) as u32 + 8
    } else {
        inner_bytes as u32 + 8
    }
}

fn write_chunk<W: Write>(mut w: W, name: &[u8], data: &[u8]) -> io::Result<()> {
    debug_assert!(name.len() == 4);

    w.write_all(name)?;
    w.write_all(&(data.len() as u32).to_le_bytes())?;
    w.write_all(data)?;
    if data.len() % 2 == 1 {
        w.write_all(&[0])?;
    }
    Ok(())
}

/// WebP Encoder.
pub struct WebPEncoder<W> {
    writer: W,
    icc_profile: Vec<u8>,
    exif_metadata: Vec<u8>,
    xmp_metadata: Vec<u8>,
    params: EncoderParams,
}

impl<W: Write> WebPEncoder<W> {
    /// Create a new encoder that writes its output to `w`.
    ///
    /// Only supports "VP8L" lossless encoding.
    pub fn new(w: W) -> Self {
        Self {
            writer: w,
            icc_profile: Vec::new(),
            exif_metadata: Vec::new(),
            xmp_metadata: Vec::new(),
            params: EncoderParams::default(),
        }
    }

    /// Set the ICC profile to use for the image.
    pub fn set_icc_profile(&mut self, icc_profile: Vec<u8>) {
        self.icc_profile = icc_profile;
    }

    /// Set the EXIF metadata to use for the image.
    pub fn set_exif_metadata(&mut self, exif_metadata: Vec<u8>) {
        self.exif_metadata = exif_metadata;
    }

    /// Set the XMP metadata to use for the image.
    pub fn set_xmp_metadata(&mut self, xmp_metadata: Vec<u8>) {
        self.xmp_metadata = xmp_metadata;
    }

    /// Set the `EncoderParams` to use.
    pub fn set_params(&mut self, params: EncoderParams) {
        self.params = params;
    }

    /// Encode image data with the indicated color type.
    ///
    /// # Panics
    ///
    /// Panics if the image data is not of the indicated dimensions.
    pub fn encode(
        mut self,
        data: &[u8],
        width: u32,
        height: u32,
        color: ColorType,
    ) -> Result<(), EncodingError> {
        let mut frame = Vec::new();
        encode_frame(&mut frame, data, width, height, color, self.params)?;

        // If the image has no metadata, it can be encoded with the "simple" WebP container format.
        if self.icc_profile.is_empty()
            && self.exif_metadata.is_empty()
            && self.xmp_metadata.is_empty()
        {
            self.writer.write_all(b"RIFF")?;
            self.writer
                .write_all(&(chunk_size(frame.len()) + 4).to_le_bytes())?;
            self.writer.write_all(b"WEBP")?;
            write_chunk(&mut self.writer, b"VP8L", &frame)?;
        } else {
            let mut total_bytes = 22 + chunk_size(frame.len());
            if !self.icc_profile.is_empty() {
                total_bytes += chunk_size(self.icc_profile.len());
            }
            if !self.exif_metadata.is_empty() {
                total_bytes += chunk_size(self.exif_metadata.len());
            }
            if !self.xmp_metadata.is_empty() {
                total_bytes += chunk_size(self.xmp_metadata.len());
            }

            let mut flags = 0;
            if !self.xmp_metadata.is_empty() {
                flags |= 1 << 2;
            }
            if !self.exif_metadata.is_empty() {
                flags |= 1 << 3;
            }
            if let ColorType::La8 | ColorType::Rgba8 = color {
                flags |= 1 << 4;
            }
            if !self.icc_profile.is_empty() {
                flags |= 1 << 5;
            }

            self.writer.write_all(b"RIFF")?;
            self.writer.write_all(&total_bytes.to_le_bytes())?;
            self.writer.write_all(b"WEBP")?;

            let mut vp8x = Vec::new();
            vp8x.write_all(&[flags])?; // flags
            vp8x.write_all(&[0; 3])?; // reserved
            vp8x.write_all(&(width - 1).to_le_bytes()[..3])?; // canvas width
            vp8x.write_all(&(height - 1).to_le_bytes()[..3])?; // canvas height
            write_chunk(&mut self.writer, b"VP8X", &vp8x)?;

            if !self.icc_profile.is_empty() {
                write_chunk(&mut self.writer, b"ICCP", &self.icc_profile)?;
            }

            write_chunk(&mut self.writer, b"VP8L", &frame)?;

            if !self.exif_metadata.is_empty() {
                write_chunk(&mut self.writer, b"EXIF", &self.exif_metadata)?;
            }

            if !self.xmp_metadata.is_empty() {
                write_chunk(&mut self.writer, b"XMP ", &self.xmp_metadata)?;
            }
        }

        Ok(())
    }
}

#[cfg(any())]
mod tests {
    use rand::RngCore;

    use super::*;

    #[test]
    fn write_webp() {
        let mut img = vec![0; 256 * 256 * 4];
        rand::thread_rng().fill_bytes(&mut img);

        let mut output = Vec::new();
        WebPEncoder::new(&mut output)
            .encode(&img, 256, 256, crate::ColorType::Rgba8)
            .unwrap();

        let mut decoder = crate::WebPDecoder::new(std::io::Cursor::new(output)).unwrap();
        let mut img2 = vec![0; 256 * 256 * 4];
        decoder.read_image(&mut img2).unwrap();
        assert_eq!(img, img2);
    }

    #[test]
    fn write_webp_exif() {
        let mut img = vec![0; 256 * 256 * 3];
        rand::thread_rng().fill_bytes(&mut img);

        let mut exif = vec![0; 10];
        rand::thread_rng().fill_bytes(&mut exif);

        let mut output = Vec::new();
        let mut encoder = WebPEncoder::new(&mut output);
        encoder.set_exif_metadata(exif.clone());
        encoder
            .encode(&img, 256, 256, crate::ColorType::Rgb8)
            .unwrap();

        let mut decoder = crate::WebPDecoder::new(std::io::Cursor::new(output)).unwrap();

        let mut img2 = vec![0; 256 * 256 * 3];
        decoder.read_image(&mut img2).unwrap();
        assert_eq!(img, img2);

        let exif2 = decoder.exif_metadata().unwrap();
        assert_eq!(Some(exif), exif2);
    }

    #[test]
    fn roundtrip_libwebp() {
        roundtrip_libwebp_params(EncoderParams::default());
        roundtrip_libwebp_params(EncoderParams {
            use_predictor_transform: false,
            ..Default::default()
        });
    }

    fn roundtrip_libwebp_params(params: EncoderParams) {
        println!("Testing {params:?}");

        let mut img = vec![0; 256 * 256 * 4];
        rand::thread_rng().fill_bytes(&mut img);

        let mut output = Vec::new();
        let mut encoder = WebPEncoder::new(&mut output);
        encoder.set_params(params.clone());
        encoder
            .encode(&img[..256 * 256 * 3], 256, 256, crate::ColorType::Rgb8)
            .unwrap();
        let decoded = webp::Decoder::new(&output).decode().unwrap();
        assert_eq!(img[..256 * 256 * 3], *decoded);

        let mut output = Vec::new();
        let mut encoder = WebPEncoder::new(&mut output);
        encoder.set_params(params.clone());
        encoder
            .encode(&img, 256, 256, crate::ColorType::Rgba8)
            .unwrap();
        let decoded = webp::Decoder::new(&output).decode().unwrap();
        assert_eq!(img, *decoded);

        let mut output = Vec::new();
        let mut encoder = WebPEncoder::new(&mut output);
        encoder.set_params(params.clone());
        encoder.set_icc_profile(vec![0; 10]);
        encoder
            .encode(&img, 256, 256, crate::ColorType::Rgba8)
            .unwrap();
        let decoded = webp::Decoder::new(&output).decode().unwrap();
        assert_eq!(img, *decoded);

        let mut output = Vec::new();
        let mut encoder = WebPEncoder::new(&mut output);
        encoder.set_params(params.clone());
        encoder.set_exif_metadata(vec![0; 10]);
        encoder
            .encode(&img, 256, 256, crate::ColorType::Rgba8)
            .unwrap();
        let decoded = webp::Decoder::new(&output).decode().unwrap();
        assert_eq!(img, *decoded);

        let mut output = Vec::new();
        let mut encoder = WebPEncoder::new(&mut output);
        encoder.set_params(params);
        encoder.set_xmp_metadata(vec![0; 7]);
        encoder.set_icc_profile(vec![0; 8]);
        encoder.set_icc_profile(vec![0; 9]);
        encoder
            .encode(&img, 256, 256, crate::ColorType::Rgba8)
            .unwrap();
        let decoded = webp::Decoder::new(&output).decode().unwrap();
        assert_eq!(img, *decoded);
    }
}

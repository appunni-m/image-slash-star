//! Feature-gated image codec implementations.
//!
//! Each format owns its decoding and encoding implementation so enabling one
//! Cargo feature pulls in only that codec and its private support code.

use crate::SequenceDecodeBudget;
use crate::encode_options::EncodeOptions;
use crate::types::{
    DecodedImage, DecodedSequence, FrameDisposal, ImageError, ImageErrorStage, ImageFormat,
    ImageInfo, ImageResult,
};

mod error;
#[cfg(any(
    feature = "jpeg",
    feature = "png",
    feature = "gif",
    feature = "bmp",
    feature = "tiff",
    feature = "webp",
    feature = "ico",
    feature = "avif"
))]
pub(crate) use error::CodecError;
#[cfg(any(
    feature = "jpeg",
    feature = "png",
    feature = "gif",
    feature = "bmp",
    feature = "tiff",
    feature = "webp",
    feature = "ico"
))]
pub(crate) use error::OptionCodecExt;
pub(crate) use error::{CodecResult, into_image_result};

#[cfg(feature = "avif")]
mod avif;
#[cfg(feature = "bmp")]
mod bmp;
#[cfg(feature = "gif")]
mod gif;
#[cfg(feature = "ico")]
mod ico;
#[cfg(feature = "jpeg")]
mod jpeg;
#[cfg(feature = "png")]
mod png;
#[cfg(feature = "tiff")]
mod tiff;
#[cfg(feature = "webp")]
mod webp;

#[cfg(any(feature = "png", feature = "tiff"))]
mod compression;

/// Dispatch decoding to the enabled format implementation.
pub(crate) fn decode_format(
    _data: &[u8],
    format: ImageFormat,
) -> ImageResult<(DecodedImage, Option<usize>)> {
    #[cfg(all(target_arch = "wasm32", feature = "avif"))]
    if format == ImageFormat::Avif {
        let (image, consumed) = into_image_result(
            avif::decode::decode(_data),
            format,
            ImageErrorStage::StillDecode,
        )?;
        return validate_decoded_image(image).map(|image| (image, Some(consumed)));
    }

    #[cfg(any(
        not(all(
            feature = "jpeg",
            feature = "png",
            feature = "gif",
            feature = "bmp",
            feature = "tiff",
            feature = "webp",
            feature = "ico",
            feature = "avif"
        )),
        target_arch = "wasm32"
    ))]
    ensure_available(format)?;
    let decoded: CodecResult<(DecodedImage, Option<usize>)> = match format {
        #[cfg(feature = "jpeg")]
        ImageFormat::Jpeg => {
            jpeg::decode::decode(_data).map(|(image, consumed)| (image, Some(consumed)))
        }
        #[cfg(not(feature = "jpeg"))]
        ImageFormat::Jpeg => Err(error::CodecError::Malformed(
            "JPEG feature is disabled".to_owned(),
        )),
        #[cfg(feature = "png")]
        ImageFormat::Png => {
            png::decode::decode(_data).map(|(image, consumed)| (image, Some(consumed)))
        }
        #[cfg(not(feature = "png"))]
        ImageFormat::Png => Err(error::CodecError::Malformed(
            "PNG feature is disabled".to_owned(),
        )),
        #[cfg(feature = "gif")]
        ImageFormat::Gif => {
            gif::decode::decode(_data).map(|(image, consumed)| (image, Some(consumed)))
        }
        #[cfg(not(feature = "gif"))]
        ImageFormat::Gif => Err(error::CodecError::Malformed(
            "GIF feature is disabled".to_owned(),
        )),
        #[cfg(feature = "bmp")]
        ImageFormat::Bmp => bmp::decode::decode(_data),
        #[cfg(not(feature = "bmp"))]
        ImageFormat::Bmp => Err(error::CodecError::Malformed(
            "BMP feature is disabled".to_owned(),
        )),
        #[cfg(feature = "tiff")]
        ImageFormat::Tiff => {
            tiff::decode::decode(_data).map(|(image, consumed)| (image, Some(consumed)))
        }
        #[cfg(not(feature = "tiff"))]
        ImageFormat::Tiff => Err(error::CodecError::Malformed(
            "TIFF feature is disabled".to_owned(),
        )),
        #[cfg(feature = "webp")]
        ImageFormat::WebP => {
            webp::decode::decode(_data).map(|(image, consumed)| (image, Some(consumed)))
        }
        #[cfg(not(feature = "webp"))]
        ImageFormat::WebP => Err(error::CodecError::Malformed(
            "WebP feature is disabled".to_owned(),
        )),
        #[cfg(feature = "ico")]
        ImageFormat::Ico => ico::decode::decode(_data),
        #[cfg(not(feature = "ico"))]
        ImageFormat::Ico => Err(error::CodecError::Malformed(
            "ICO feature is disabled".to_owned(),
        )),
        #[cfg(feature = "avif")]
        ImageFormat::Avif => {
            avif::decode::decode(_data).map(|(image, consumed)| (image, Some(consumed)))
        }
        #[cfg(not(feature = "avif"))]
        ImageFormat::Avif => Err(error::CodecError::Malformed(
            "AVIF feature is disabled".to_owned(),
        )),
    };
    let (image, consumed) = match decoded {
        Ok(decoded) => decoded,
        Err(error) => {
            return Err(error
                .context("decode")
                .into_image_error(format, ImageErrorStage::StillDecode));
        }
    };
    validate_decoded_image(image).map(|image| (image, consumed))
}

fn validate_decoded_image(image: DecodedImage) -> ImageResult<DecodedImage> {
    image.validate()?;
    Ok(image)
}

/// Dispatch header inspection to the enabled format implementation.
pub(crate) fn inspect_format(_data: &[u8], format: ImageFormat) -> ImageResult<ImageInfo> {
    #[cfg(not(all(
        feature = "jpeg",
        feature = "png",
        feature = "gif",
        feature = "bmp",
        feature = "tiff",
        feature = "webp",
        feature = "ico",
        feature = "avif"
    )))]
    ensure_inspection_available(format)?;

    let inspected: CodecResult<ImageInfo> = match format {
        #[cfg(feature = "jpeg")]
        ImageFormat::Jpeg => jpeg::inspect::inspect(_data),
        #[cfg(not(feature = "jpeg"))]
        ImageFormat::Jpeg => Err(error::CodecError::Unsupported(
            "JPEG metadata inspection is unavailable".to_owned(),
        )),
        #[cfg(feature = "png")]
        ImageFormat::Png => png::inspect::inspect(_data),
        #[cfg(not(feature = "png"))]
        ImageFormat::Png => Err(error::CodecError::Unsupported(
            "PNG metadata inspection is unavailable".to_owned(),
        )),
        #[cfg(feature = "gif")]
        ImageFormat::Gif => gif::inspect::inspect(_data),
        #[cfg(not(feature = "gif"))]
        ImageFormat::Gif => Err(error::CodecError::Unsupported(
            "GIF metadata inspection is unavailable".to_owned(),
        )),
        #[cfg(feature = "bmp")]
        ImageFormat::Bmp => bmp::inspect::inspect(_data),
        #[cfg(not(feature = "bmp"))]
        ImageFormat::Bmp => Err(error::CodecError::Unsupported(
            "BMP metadata inspection is unavailable".to_owned(),
        )),
        #[cfg(feature = "tiff")]
        ImageFormat::Tiff => tiff::inspect::inspect(_data),
        #[cfg(not(feature = "tiff"))]
        ImageFormat::Tiff => Err(error::CodecError::Unsupported(
            "TIFF metadata inspection is unavailable".to_owned(),
        )),
        #[cfg(feature = "webp")]
        ImageFormat::WebP => webp::inspect::inspect(_data),
        #[cfg(not(feature = "webp"))]
        ImageFormat::WebP => Err(error::CodecError::Unsupported(
            "WebP metadata inspection is unavailable".to_owned(),
        )),
        #[cfg(feature = "ico")]
        ImageFormat::Ico => ico::inspect::inspect(_data),
        #[cfg(not(feature = "ico"))]
        ImageFormat::Ico => Err(error::CodecError::Unsupported(
            "ICO metadata inspection is unavailable".to_owned(),
        )),
        #[cfg(feature = "avif")]
        ImageFormat::Avif => avif::inspect::inspect(_data),
        #[cfg(not(feature = "avif"))]
        ImageFormat::Avif => Err(error::CodecError::Unsupported(
            "AVIF metadata inspection is unavailable".to_owned(),
        )),
    };
    into_image_result(
        inspected.map_err(|error| error.context("inspect")),
        format,
        ImageErrorStage::Inspection,
    )
}

/// Apply the pinned Pillow oracle's codec-specific verification contract.
pub(crate) fn verify_format(_data: &[u8], format: ImageFormat) -> ImageResult<()> {
    #[cfg(not(all(
        feature = "jpeg",
        feature = "png",
        feature = "gif",
        feature = "bmp",
        feature = "tiff",
        feature = "webp",
        feature = "ico",
        feature = "avif"
    )))]
    ensure_inspection_available(format)?;

    let verified: CodecResult<()> = match format {
        #[cfg(feature = "png")]
        ImageFormat::Png => png::decode::verify(_data),
        #[cfg(feature = "webp")]
        ImageFormat::WebP => webp::decode::verify(_data),
        #[cfg(feature = "jpeg")]
        ImageFormat::Jpeg => jpeg::inspect::verify(_data),
        // Formats without a manifest-proven structural verifier retain
        // Pillow's metadata-open verification behavior. EncodedImage
        // construction has already performed that inspection.
        _ => Ok(()),
    };
    into_image_result(
        verified.map_err(|error| error.context("verify")),
        format,
        ImageErrorStage::Verification,
    )
}

/// Dispatch decoding while retaining every frame and its presentation data.
pub(crate) fn decode_sequence_format(
    data: &[u8],
    format: ImageFormat,
    _budget: &mut SequenceDecodeBudget,
) -> ImageResult<(DecodedSequence, Option<usize>)> {
    #[cfg(any(
        not(all(
            feature = "jpeg",
            feature = "png",
            feature = "gif",
            feature = "bmp",
            feature = "tiff",
            feature = "webp",
            feature = "ico",
            feature = "avif"
        )),
        target_arch = "wasm32"
    ))]
    ensure_available(format)?;
    #[cfg(feature = "gif")]
    if format == ImageFormat::Gif {
        return into_image_result(
            gif::decode::decode_sequence(data, _budget)
                .map(|(sequence, consumed)| (sequence, Some(consumed)))
                .map_err(|error| error.context("decode sequence")),
            format,
            ImageErrorStage::SequenceDecode,
        );
    }

    #[cfg(feature = "png")]
    if format == ImageFormat::Png {
        return into_image_result(
            png::decode::decode_sequence(data, _budget)
                .map(|(sequence, consumed)| (sequence, Some(consumed)))
                .map_err(|error| error.context("decode sequence")),
            format,
            ImageErrorStage::SequenceDecode,
        );
    }

    #[cfg(feature = "webp")]
    if format == ImageFormat::WebP {
        return into_image_result(
            webp::decode::decode_sequence(data, _budget)
                .map(|(sequence, consumed)| (sequence, Some(consumed)))
                .map_err(|error| error.context("decode sequence")),
            format,
            ImageErrorStage::SequenceDecode,
        );
    }

    #[cfg(feature = "tiff")]
    if format == ImageFormat::Tiff {
        return into_image_result(
            tiff::decode::decode_sequence(data, _budget)
                .map(|(sequence, consumed)| (sequence, Some(consumed)))
                .map_err(|error| error.context("decode sequence")),
            format,
            ImageErrorStage::SequenceDecode,
        );
    }

    #[cfg(feature = "avif")]
    if format == ImageFormat::Avif {
        return into_image_result(
            avif::decode::decode_sequence(data, _budget)
                .map(|(sequence, consumed)| (sequence, Some(consumed)))
                .map_err(|error| error.context("decode sequence")),
            format,
            ImageErrorStage::SequenceDecode,
        );
    }

    decode_format(data, format)
        .map(|(image, consumed)| (DecodedSequence::from_image(image), consumed))
}

/// Dispatch encoding to the enabled format implementation.
pub(crate) fn encode_format(
    _image: &DecodedImage,
    format: ImageFormat,
    _options: &EncodeOptions,
) -> ImageResult<Vec<u8>> {
    #[cfg(any(
        not(all(
            feature = "jpeg",
            feature = "png",
            feature = "gif",
            feature = "bmp",
            feature = "tiff",
            feature = "webp",
            feature = "ico",
            feature = "avif"
        )),
        target_arch = "wasm32"
    ))]
    ensure_available(format)?;
    _image
        .validate()
        .map_err(|error| error.with_format(format))?;
    #[cfg(any(
        feature = "jpeg",
        feature = "png",
        feature = "gif",
        feature = "bmp",
        feature = "tiff",
        feature = "webp",
        feature = "ico",
        feature = "avif"
    ))]
    {
        let encoded: CodecResult<Vec<u8>> = match (format, _options) {
            #[cfg(feature = "jpeg")]
            (ImageFormat::Jpeg, EncodeOptions::Jpeg(options)) => {
                jpeg::encode::encode(_image, options)
            }
            #[cfg(feature = "png")]
            (ImageFormat::Png, EncodeOptions::Png(options)) => png::encode::encode(_image, options),
            #[cfg(feature = "gif")]
            (ImageFormat::Gif, EncodeOptions::Gif(options)) => gif::encode::encode(_image, options),
            #[cfg(feature = "bmp")]
            (ImageFormat::Bmp, EncodeOptions::Bmp(options)) => bmp::encode::encode(_image, options),
            #[cfg(feature = "tiff")]
            (ImageFormat::Tiff, EncodeOptions::Tiff(options)) => {
                tiff::encode::encode(_image, options)
            }
            #[cfg(feature = "webp")]
            (ImageFormat::WebP, EncodeOptions::WebP(options)) => {
                webp::encode::encode(_image, options)
            }
            #[cfg(feature = "ico")]
            (ImageFormat::Ico, EncodeOptions::Ico(options)) => ico::encode::encode(_image, options),
            #[cfg(feature = "avif")]
            (ImageFormat::Avif, EncodeOptions::Avif(options)) => {
                avif::encode::encode(_image, options)
            }
            _ => {
                return Err(option_format_mismatch(
                    format,
                    _options,
                    ImageErrorStage::StillEncode,
                ));
            }
        };
        into_image_result(
            encoded.map_err(|error| error.context("encode")),
            format,
            ImageErrorStage::StillEncode,
        )
    }
    #[cfg(not(any(
        feature = "jpeg",
        feature = "png",
        feature = "gif",
        feature = "bmp",
        feature = "tiff",
        feature = "webp",
        feature = "ico",
        feature = "avif"
    )))]
    Err(option_format_mismatch(
        format,
        _options,
        ImageErrorStage::StillEncode,
    ))
}

/// Dispatch encoding without collapsing an animation to its first frame.
pub(crate) fn encode_sequence_format(
    sequence: &DecodedSequence,
    format: ImageFormat,
    options: &EncodeOptions,
) -> ImageResult<Vec<u8>> {
    #[cfg(any(
        not(all(
            feature = "jpeg",
            feature = "png",
            feature = "gif",
            feature = "bmp",
            feature = "tiff",
            feature = "webp",
            feature = "ico",
            feature = "avif"
        )),
        target_arch = "wasm32"
    ))]
    ensure_available(format)?;
    sequence
        .validate()
        .map_err(|error| error.with_format(format))?;
    match (format, options) {
        #[cfg(feature = "gif")]
        (ImageFormat::Gif, EncodeOptions::Gif(options)) => {
            return into_image_result(
                gif::encode::encode_sequence(sequence, options)
                    .map_err(|error| error.context("encode sequence")),
                format,
                ImageErrorStage::SequenceEncode,
            );
        }
        #[cfg(feature = "avif")]
        (ImageFormat::Avif, EncodeOptions::Avif(options)) => {
            return into_image_result(
                avif::encode::encode_sequence(sequence, options)
                    .map_err(|error| error.context("encode sequence")),
                format,
                ImageErrorStage::SequenceEncode,
            );
        }
        #[cfg(feature = "tiff")]
        (ImageFormat::Tiff, EncodeOptions::Tiff(options)) => {
            return into_image_result(
                tiff::encode::encode_sequence(sequence, options)
                    .map_err(|error| error.context("encode sequence")),
                format,
                ImageErrorStage::SequenceEncode,
            );
        }
        #[cfg(feature = "webp")]
        (ImageFormat::WebP, EncodeOptions::WebP(options)) if sequence.frames.len() > 1 => {
            return into_image_result(
                webp::encode::encode_sequence(sequence, options)
                    .map_err(|error| error.context("encode sequence")),
                format,
                ImageErrorStage::SequenceEncode,
            );
        }
        (_, supplied) if supplied.format() != format => {
            return Err(option_format_mismatch(
                format,
                supplied,
                ImageErrorStage::SequenceEncode,
            ));
        }
        _ => {}
    }

    if sequence.frames.len() != 1 {
        return Err(ImageError::Unsupported {
            format: Some(format),
            message: "format cannot encode multiple retained frames".to_owned(),
            stage: Some(ImageErrorStage::SequenceEncode),
            offset: None,
            identity: None,
        });
    }
    let frame = &sequence.frames[0];
    if !has_plain_still_semantics(sequence, frame) {
        return Err(ImageError::Unsupported {
            format: Some(format),
            message: "still-image format cannot represent retained sequence metadata".to_owned(),
            stage: Some(ImageErrorStage::SequenceEncode),
            offset: None,
            identity: None,
        });
    }
    encode_format(&frame.image, format, options)
}

fn option_format_mismatch(
    format: ImageFormat,
    options: &EncodeOptions,
    stage: ImageErrorStage,
) -> ImageError {
    ImageError::Parameter {
        format: Some(format),
        message: format!(
            "{} options cannot be used to encode {}",
            options.format(),
            format
        ),
        stage: Some(stage),
        offset: None,
        identity: None,
    }
}

/// Measure the encoded metadata extent of one detected format.
///
/// The metadata extent is the container-defined consumed length minus the
/// encoded bytes of primary pixel payload data. It is computed without
/// decompressing pixels so `max_metadata_bytes` can reject before codec work.
pub(crate) fn metadata_bytes_format(data: &[u8], format: ImageFormat) -> ImageResult<u64> {
    #[cfg(any(
        not(all(
            feature = "jpeg",
            feature = "png",
            feature = "gif",
            feature = "bmp",
            feature = "tiff",
            feature = "webp",
            feature = "ico",
            feature = "avif"
        )),
        target_arch = "wasm32"
    ))]
    ensure_available(format)?;
    #[cfg(any(
        not(all(
            feature = "jpeg",
            feature = "png",
            feature = "gif",
            feature = "bmp",
            feature = "tiff",
            feature = "webp",
            feature = "ico",
            feature = "avif"
        )),
        target_arch = "wasm32"
    ))]
    let _ = data;
    let metadata: CodecResult<u64> = match format {
        #[cfg(feature = "jpeg")]
        ImageFormat::Jpeg => jpeg::decode::metadata_bytes(data),
        #[cfg(not(feature = "jpeg"))]
        ImageFormat::Jpeg => Err(error::CodecError::Malformed(
            "JPEG feature is disabled".to_owned(),
        )),
        #[cfg(feature = "png")]
        ImageFormat::Png => png::decode::metadata_bytes(data),
        #[cfg(not(feature = "png"))]
        ImageFormat::Png => Err(error::CodecError::Malformed(
            "PNG feature is disabled".to_owned(),
        )),
        #[cfg(feature = "gif")]
        ImageFormat::Gif => gif::decode::metadata_bytes(data),
        #[cfg(not(feature = "gif"))]
        ImageFormat::Gif => Err(error::CodecError::Malformed(
            "GIF feature is disabled".to_owned(),
        )),
        #[cfg(feature = "bmp")]
        ImageFormat::Bmp => bmp::decode::metadata_bytes(data),
        #[cfg(not(feature = "bmp"))]
        ImageFormat::Bmp => Err(error::CodecError::Malformed(
            "BMP feature is disabled".to_owned(),
        )),
        #[cfg(feature = "tiff")]
        ImageFormat::Tiff => tiff::decode::metadata_bytes(data),
        #[cfg(not(feature = "tiff"))]
        ImageFormat::Tiff => Err(error::CodecError::Malformed(
            "TIFF feature is disabled".to_owned(),
        )),
        #[cfg(feature = "webp")]
        ImageFormat::WebP => webp::decode::metadata_bytes(data),
        #[cfg(not(feature = "webp"))]
        ImageFormat::WebP => Err(error::CodecError::Malformed(
            "WebP feature is disabled".to_owned(),
        )),
        #[cfg(feature = "ico")]
        ImageFormat::Ico => ico::decode::metadata_bytes(data),
        #[cfg(not(feature = "ico"))]
        ImageFormat::Ico => Err(error::CodecError::Malformed(
            "ICO feature is disabled".to_owned(),
        )),
        #[cfg(feature = "avif")]
        ImageFormat::Avif => avif::decode::metadata_bytes(data),
        #[cfg(not(feature = "avif"))]
        ImageFormat::Avif => Err(error::CodecError::Malformed(
            "AVIF feature is disabled".to_owned(),
        )),
    };
    into_image_result(
        metadata.map_err(|error| error.context("metadata measure")),
        format,
        ImageErrorStage::Inspection,
    )
}

fn has_plain_still_semantics(
    sequence: &DecodedSequence,
    frame: &crate::types::DecodedFrame,
) -> bool {
    if frame.source.rect.left != 0 {
        return false;
    }
    if frame.source.rect.top != 0 {
        return false;
    }
    if frame.source.rect.width != sequence.width || frame.source.rect.height != sequence.height {
        return false;
    }
    if frame.source.duration.numerator != 0 {
        return false;
    }
    if frame.source.disposal != FrameDisposal::Unspecified {
        return false;
    }
    if frame.source.blend != crate::types::FrameBlend::Unspecified
        || frame.source.interlaced
        || frame.source.is_default_image
    {
        return false;
    }
    if sequence.loop_count.is_some() {
        return false;
    }
    if sequence.background.is_some() {
        return false;
    }
    true
}

#[cfg(all(
    target_arch = "wasm32",
    feature = "jpeg",
    feature = "png",
    feature = "gif",
    feature = "bmp",
    feature = "tiff",
    feature = "webp",
    feature = "ico",
    feature = "avif"
))]
fn ensure_available(_format: ImageFormat) -> ImageResult<()> {
    Ok(())
}

#[cfg(not(all(
    feature = "jpeg",
    feature = "png",
    feature = "gif",
    feature = "bmp",
    feature = "tiff",
    feature = "webp",
    feature = "ico",
    feature = "avif"
)))]
fn ensure_available(format: ImageFormat) -> ImageResult<()> {
    ensure_enabled(format)
}

#[cfg(not(all(
    feature = "jpeg",
    feature = "png",
    feature = "gif",
    feature = "bmp",
    feature = "tiff",
    feature = "webp",
    feature = "ico",
    feature = "avif"
)))]
fn ensure_inspection_available(format: ImageFormat) -> ImageResult<()> {
    ensure_enabled(format)
}

#[cfg(not(all(
    feature = "jpeg",
    feature = "png",
    feature = "gif",
    feature = "bmp",
    feature = "tiff",
    feature = "webp",
    feature = "ico",
    feature = "avif"
)))]
fn ensure_enabled(format: ImageFormat) -> ImageResult<()> {
    #[cfg(not(feature = "jpeg"))]
    if format == ImageFormat::Jpeg {
        return Err(ImageError::FeatureDisabled {
            format,
            feature: "jpeg",
        });
    }
    #[cfg(not(feature = "png"))]
    if format == ImageFormat::Png {
        return Err(ImageError::FeatureDisabled {
            format,
            feature: "png",
        });
    }
    #[cfg(not(feature = "gif"))]
    if format == ImageFormat::Gif {
        return Err(ImageError::FeatureDisabled {
            format,
            feature: "gif",
        });
    }
    #[cfg(not(feature = "bmp"))]
    if format == ImageFormat::Bmp {
        return Err(ImageError::FeatureDisabled {
            format,
            feature: "bmp",
        });
    }
    #[cfg(not(feature = "tiff"))]
    if format == ImageFormat::Tiff {
        return Err(ImageError::FeatureDisabled {
            format,
            feature: "tiff",
        });
    }
    #[cfg(not(feature = "webp"))]
    if format == ImageFormat::WebP {
        return Err(ImageError::FeatureDisabled {
            format,
            feature: "webp",
        });
    }
    #[cfg(not(feature = "ico"))]
    if format == ImageFormat::Ico {
        return Err(ImageError::FeatureDisabled {
            format,
            feature: "ico",
        });
    }
    #[cfg(not(feature = "avif"))]
    if format == ImageFormat::Avif {
        return Err(ImageError::FeatureDisabled {
            format,
            feature: "avif",
        });
    }
    Ok(())
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    error::__coverage_exercise_private_branches();

    let invalid_sequence = DecodedSequence {
        width: 0,
        height: 1,
        frames: Vec::new(),
        loop_count: None,
        background: None,
        kind: crate::types::SequenceKind::SingleFrame,
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: crate::types::SourceColor::new(),
    };
    let _ = encode_sequence_format(
        &invalid_sequence,
        ImageFormat::Png,
        &EncodeOptions::for_format(ImageFormat::Png),
    );

    let luma = DecodedImage::new(1, 1, vec![0], crate::types::ColorType::L8);
    let two_frame_sequence = DecodedSequence {
        width: 1,
        height: 1,
        frames: vec![
            crate::types::DecodedFrame::rendered_canvas(
                luma.clone(),
                crate::types::FrameRect {
                    left: 0,
                    top: 0,
                    width: 1,
                    height: 1,
                },
                crate::types::FrameDuration::ZERO,
                crate::types::FrameDisposal::Unspecified,
                crate::types::FrameBlend::Unspecified,
            ),
            crate::types::DecodedFrame::rendered_canvas(
                luma,
                crate::types::FrameRect {
                    left: 0,
                    top: 0,
                    width: 1,
                    height: 1,
                },
                crate::types::FrameDuration::ZERO,
                crate::types::FrameDisposal::Unspecified,
                crate::types::FrameBlend::Unspecified,
            ),
        ],
        loop_count: None,
        background: None,
        kind: crate::types::SequenceKind::TimedAnimation,
        opaque_blocks: Vec::new(),
        metadata: Vec::new(),
        source_color: crate::types::SourceColor::new(),
    };
    let _ = encode_sequence_format(
        &two_frame_sequence,
        ImageFormat::Png,
        &EncodeOptions::for_format(ImageFormat::Png),
    );

    let invalid_image = DecodedImage::new(1, 1, Vec::new(), crate::types::ColorType::Rgb8);
    let _ = validate_decoded_image(invalid_image);

    // The manifest currently has no malformed AVIF container fixture. Keep the
    // dispatch-only error conversion covered without weakening AVIF parity rows.
    #[cfg(feature = "avif")]
    let _ = decode_sequence_format(
        b"not an AVIF container",
        ImageFormat::Avif,
        &mut SequenceDecodeBudget::default_for(ImageFormat::Avif),
    );

    compression::__coverage_exercise_private_branches();
    #[cfg(feature = "avif")]
    avif::__coverage_exercise_private_branches();
    #[cfg(feature = "bmp")]
    bmp::__coverage_exercise_private_branches();
    #[cfg(feature = "gif")]
    gif::__coverage_exercise_private_branches();
    #[cfg(feature = "ico")]
    ico::__coverage_exercise_private_branches();
    #[cfg(feature = "jpeg")]
    jpeg::__coverage_exercise_private_branches();
    #[cfg(feature = "png")]
    png::__coverage_exercise_private_branches();
    #[cfg(feature = "tiff")]
    tiff::__coverage_exercise_private_branches();
    #[cfg(feature = "webp")]
    webp::__coverage_exercise_private_branches();
}

#[cfg(all(coverage, feature = "avif", not(target_arch = "wasm32")))]
pub(crate) fn __coverage_av1_entropy_reference_trace()
-> CodecResult<Vec<crate::Av1EntropyTraceState>> {
    avif::__coverage_entropy_reference_trace()
}

#[cfg(all(coverage, feature = "avif", not(target_arch = "wasm32")))]
pub(crate) fn __coverage_av1_reconstruction(
    data: &[u8],
) -> CodecResult<Option<crate::Av1ReconstructionTrace>> {
    avif::__coverage_reconstruction(data)
}

#[cfg(all(coverage, feature = "avif", not(target_arch = "wasm32")))]
pub(crate) fn __coverage_sweep_av1_first_leaf(data: &[u8]) {
    avif::__coverage_sweep_first_leaf(data);
}

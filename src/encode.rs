//! Compatibility facade for the format-first codec modules.

pub use crate::codecs::{encode_format, encode_sequence_format};

#[cfg(feature = "avif")]
pub use crate::codecs::avif::encode as avif;
#[cfg(feature = "bmp")]
pub use crate::codecs::bmp::encode as bmp;
#[cfg(feature = "gif")]
pub use crate::codecs::gif::encode as gif;
#[cfg(feature = "ico")]
pub use crate::codecs::ico::encode as ico;
#[cfg(feature = "jpeg")]
pub use crate::codecs::jpeg::encode as jpeg;
#[cfg(feature = "png")]
pub use crate::codecs::png::encode as png;
#[cfg(feature = "tiff")]
pub use crate::codecs::tiff::encode as tiff;
#[cfg(feature = "webp")]
pub use crate::codecs::webp::encode as webp;

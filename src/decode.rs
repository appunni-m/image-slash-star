//! Compatibility facade for the format-first codec modules.

pub use crate::codecs::{decode_format, decode_sequence_format};

#[cfg(feature = "avif")]
pub use crate::codecs::avif::decode as avif;
#[cfg(feature = "bmp")]
pub use crate::codecs::bmp::decode as bmp;
#[cfg(feature = "gif")]
pub use crate::codecs::gif::decode as gif;
#[cfg(feature = "ico")]
pub use crate::codecs::ico::decode as ico;
#[cfg(feature = "jpeg")]
pub use crate::codecs::jpeg::decode as jpeg;
#[cfg(feature = "png")]
pub use crate::codecs::png::decode as png;
#[cfg(feature = "tiff")]
pub use crate::codecs::tiff::decode as tiff;
#[cfg(feature = "webp")]
pub use crate::codecs::webp::decode as webp;

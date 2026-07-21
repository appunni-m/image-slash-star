//! Core traits for the pillow-rs-image type system.
//!
//! Split into focused submodules:
//! - `primitive`: numeric foundations (`Primitive`, `Enlargeable`, `EncodableLayout`)
//! - `pixel`: pixel operations (`Pixel`)
//! - `view`: image viewing/mutation (`GenericImageView`, `GenericImage`)

pub(crate) mod pixel;
pub(crate) mod primitive;
pub(crate) mod view;

// Public re-exports for the crate
pub use self::pixel::Pixel;
pub use self::primitive::{EncodableLayout, Enlargeable, Primitive};
pub use self::view::{GenericImage, GenericImageView};

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    pixel::__coverage_exercise_private_branches();
    view::__coverage_exercise_private_branches();
}

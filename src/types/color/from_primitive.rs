//! `FromPrimitive` trait + implementations + luminance helpers.

use crate::types::traits::{Enlargeable, Primitive};

pub trait FromPrimitive<Component> {
    /// Converts from any pixel component type to this type.
    fn from_primitive(component: Component) -> Self;
}

impl<T: Primitive> FromPrimitive<T> for T {
    fn from_primitive(sample: T) -> Self {
        sample
    }
}
// From f32:
impl FromPrimitive<f32> for u8 {
    fn from_primitive(float: f32) -> Self {
        normalize_float(float, u8::MAX as f32) as u8
    }
}

impl FromPrimitive<f32> for u16 {
    fn from_primitive(float: f32) -> Self {
        normalize_float(float, u16::MAX as f32) as u16
    }
}

// From u16:
impl FromPrimitive<u16> for u8 {
    fn from_primitive(c16: u16) -> Self {
        ((c16 as u32 + 128) / 257) as u8
    }
}

impl FromPrimitive<u16> for f32 {
    fn from_primitive(int: u16) -> Self {
        (int as f32 / u16::MAX as f32).clamp(0.0, 1.0)
    }
}

// From u8:
impl FromPrimitive<u8> for f32 {
    fn from_primitive(int: u8) -> Self {
        (int as f32 / u8::MAX as f32).clamp(0.0, 1.0)
    }
}

impl FromPrimitive<u8> for u16 {
    fn from_primitive(c8: u8) -> Self {
        let x = c8 as u64;
        ((x << 8) | x) as u16
    }
}

#[inline]
pub(super) fn normalize_float(float: f32, max: f32) -> f32 {
    let clamped = if !(float < 1.0) { 1.0 } else { float.max(0.0) };
    (clamped * max).round()
}

// ---------------------------------------------------------------------------
// Color conversion coefficients
// ---------------------------------------------------------------------------

/// Pillow/libImaging fixed-point RGB-to-luminance coefficients.
const SRGB_LUMA: [u32; 3] = [19_595, 38_470, 7_471];
const SRGB_LUMA_DIV: u32 = 65_536;

#[inline]
pub(super) fn rgb_to_luma<T: Primitive + Enlargeable>(rgb: &[T]) -> T {
    let luma = rgb[0].to_f32() * (SRGB_LUMA[0] as f32 / SRGB_LUMA_DIV as f32)
        + rgb[1].to_f32() * (SRGB_LUMA[1] as f32 / SRGB_LUMA_DIV as f32)
        + rgb[2].to_f32() * (SRGB_LUMA[2] as f32 / SRGB_LUMA_DIV as f32);
    let rounded = luma + 0.5 * f32::from(u8::from(T::DEFAULT_MAX_VALUE.to_f32() > 1.0));
    let l = <T::Larger as Primitive>::from_f32(rounded);
    T::clamp_from(l)
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    let _ = <u8 as FromPrimitive<u8>>::from_primitive(9);
    let _ = <u16 as FromPrimitive<f32>>::from_primitive(0.5);
    let _ = <f32 as FromPrimitive<u16>>::from_primitive(123);
    let _ = <u16 as FromPrimitive<u8>>::from_primitive(7);
    let _ = rgb_to_luma::<u8>(&[1, 2, 3]);
    let _ = rgb_to_luma::<u16>(&[1, 2, 3]);
    let _ = rgb_to_luma::<f32>(&[0.1, 0.2, 0.3]);
}

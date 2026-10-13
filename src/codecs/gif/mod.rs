//! GIF codec.

pub mod decode;
pub mod encode;
pub mod inspect;

fn is_identity_grayscale_palette(palette: &[u8]) -> bool {
    let (entries, remainder) = palette.as_chunks::<3>();
    let complete = remainder.is_empty();
    let identity = entries.iter().enumerate().all(|(index, entry)| {
        let Ok(value) = u8::try_from(index) else {
            return false;
        };
        *entry == [value; 3]
    });
    identity && complete
}

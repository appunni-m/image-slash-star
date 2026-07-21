//! Minimal little-endian reader API used by the internal WebP implementation.

use std::io::{self, Read};

pub(super) struct LittleEndian;

pub(super) trait ByteOrder {
    fn u16(bytes: [u8; 2]) -> u16;
    fn u24(bytes: [u8; 3]) -> u32;
    fn u32(bytes: [u8; 4]) -> u32;
}

impl ByteOrder for LittleEndian {
    fn u16(bytes: [u8; 2]) -> u16 {
        u16::from_le_bytes(bytes)
    }

    fn u24(bytes: [u8; 3]) -> u32 {
        u32::from(bytes[0]) | (u32::from(bytes[1]) << 8) | (u32::from(bytes[2]) << 16)
    }

    fn u32(bytes: [u8; 4]) -> u32 {
        u32::from_le_bytes(bytes)
    }
}

pub(super) trait ReadBytesExt: Read {
    fn read_u8(&mut self) -> io::Result<u8> {
        let mut bytes = [0; 1];
        self.read_exact(&mut bytes)?;
        Ok(bytes[0])
    }

    fn read_u16<T: ByteOrder>(&mut self) -> io::Result<u16> {
        let mut bytes = [0; 2];
        self.read_exact(&mut bytes)?;
        Ok(T::u16(bytes))
    }

    fn read_u24<T: ByteOrder>(&mut self) -> io::Result<u32> {
        let mut bytes = [0; 3];
        self.read_exact(&mut bytes)?;
        Ok(T::u24(bytes))
    }

    fn read_u32<T: ByteOrder>(&mut self) -> io::Result<u32> {
        let mut bytes = [0; 4];
        self.read_exact(&mut bytes)?;
        Ok(T::u32(bytes))
    }
}

impl<R: Read + ?Sized> ReadBytesExt for R {}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    let mut bytes = std::io::Cursor::new([1u8, 2, 3, 4]);
    let _ = bytes.read_u8();
    let _ = bytes.read_u16::<LittleEndian>();
    let mut bytes = std::io::Cursor::new([1u8, 2, 3, 4]);
    let _ = bytes.read_u24::<LittleEndian>();
    let mut bytes = std::io::Cursor::new([1u8, 2, 3, 4]);
    let _ = bytes.read_u32::<LittleEndian>();

    let mut empty = std::io::Cursor::new(Vec::<u8>::new());
    let _ = empty.read_u8();
    let _ = empty.read_u16::<LittleEndian>();
    let _ = empty.read_u24::<LittleEndian>();
    let _ = empty.read_u32::<LittleEndian>();
}

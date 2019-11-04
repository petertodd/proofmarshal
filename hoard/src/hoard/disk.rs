use std::mem;
use std::slice;
use std::io::{self, Write};
use core::convert::{TryFrom, TryInto};
use core::borrow::Borrow;
use core::cmp;

use persist::{Le, Persist, Validate, MaybeValid, Valid};

const MAGIC: [u8;15] = [0;15];
const MAX_VERSION: u8 = 0;

fn try_split_at<T, E>(slice: &[T], idx: usize) -> Result<(&[T], &[T]), Error<E>> {
    if idx < slice.len() {
        Ok(slice.split_at(idx))
    } else {
        Err(Error::Truncated)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error<E> {
    Truncated,
    Other(E),
}

impl<E> Error<E> {
    fn map<E2>(self, f: impl FnOnce(E) -> E2) -> Error<E2> {
        match self {
            Self::Truncated => Error::Truncated,
            Self::Other(e) => Error::Other(f(e)),
        }
    }
}

impl<E> From<E> for Error<E> {
    fn from(other: E) -> Self {
        Self::Other(other)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct Header {
    pub(crate) magic: [u8;15],
    pub(crate) version: u8,
    pub(crate) first_word: Word,
}

impl Default for Header {
    fn default() -> Self {
        Self {
            magic: MAGIC,
            version: MAX_VERSION,
            first_word: 0.into(),
        }
    }
}

impl Persist for Header {
    #[inline]
    fn write_canonical<W: Write>(&self, mut dst: W) -> io::Result<W> {
        dst.write_all(&self.magic)?;
        let dst = self.version.write_canonical(dst)?;
        let dst = self.first_word.write_canonical(dst)?;
        Ok(dst)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ValidateHeaderError {
    Magic,
    Version(u8),
}

impl<V: ?Sized> Validate<V> for Header {
    type Error = ValidateHeaderError;

    fn validate<'a>(maybe: MaybeValid<'a, Self>, _: &mut V) -> Result<Valid<'a, Self>, Self::Error> {
        let this = unsafe { maybe.assume_valid() };

        if this.magic != MAGIC || this.first_word != Le::new(0) {
            Err(ValidateHeaderError::Magic)
        } else if this.version > MAX_VERSION {
            Err(ValidateHeaderError::Version(this.version))
        } else {
            Ok(this)
        }
    }
}

pub type Word = Le<u64>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PacketDiscriminant {
    Root = 0,
    Blob = 1,
}

impl Persist for PacketDiscriminant {
    #[inline]
    fn write_canonical<W: Write>(&self, mut dst: W) -> io::Result<W> {
        dst.write_all(&[*self as u8])?;
        Ok(dst)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ValidatePacketDiscriminantError(u8);

impl<V: ?Sized> Validate<V> for PacketDiscriminant {
    type Error = ValidatePacketDiscriminantError;

    #[inline]
    fn validate<'a>(maybe: MaybeValid<'a, Self>, _: &mut V) -> Result<Valid<'a, Self>, Self::Error> {
        match maybe[0] {
            0 | 1 => unsafe { Ok(maybe.assume_valid()) },
            x => Err(ValidatePacketDiscriminantError(x)),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Packet<B: ?Sized = [u8]> {
    discriminant: PacketDiscriminant,
    payload: B,
}

pub type PacketHeader = Packet<()>;

impl<B: Persist> Persist for Packet<B> {
    #[inline]
    fn write_canonical<W: Write>(&self, dst: W) -> io::Result<W> {
        let dst = self.discriminant.write_canonical(dst)?;
        let dst = self.payload.write_canonical(dst)?;
        Ok(dst)
    }
}

impl Packet<BlobHeader> {
    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            slice::from_raw_parts(self as *const Self as *const u8,
                                  mem::size_of::<Self>())
        }
    }
}

impl<B: AsRef<[u8]>> Packet<Blob<B>> {
    pub fn header(&self) -> Packet<BlobHeader> {
        Packet {
            discriminant: self.discriminant,
            payload: self.payload.header(),
        }
    }

    pub fn padding_required(&self, initial_offset: u64) -> usize {
        let words = calc_padding_words_required(initial_offset,
                                                self.header().as_bytes(),
                                                self.payload.bytes.as_ref());
        words * mem::size_of::<Word>()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ValidatePacketError {
    Discriminant(ValidatePacketDiscriminantError),
    Blob(ValidateBlobError),
}

impl Packet {
    pub fn validate(buf: &[u8]) -> Result<(&Self, &[u8]), Error<ValidatePacketError>> {
        let (header, rest) = try_split_at(buf, mem::size_of::<PacketDiscriminant>())?;
        let d = PacketDiscriminant::validate(header.try_into().unwrap(), &mut ())
                                   .map_err(|e| ValidatePacketError::Discriminant(e))?;

        let rest = match *d {
            PacketDiscriminant::Root => {
                unimplemented!()
            },
            PacketDiscriminant::Blob => {
                let (_blob, rest) = Blob::validate(rest).map_err(|e| e.map(|e| ValidatePacketError::Blob(e)))?;
                rest
            },
        };

        let this = unsafe {
            mem::transmute::<&[u8], &Self>(
                slice::from_raw_parts(buf.as_ptr(),
                                      buf.len() - rest.len())
            )
        };
        Ok((this, rest))
    }
}

#[derive(Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Blob<B: ?Sized = [u8]> {
    len: Le<u16>,
    checksum: Le<u16>,
    bytes: B,
}

pub type BlobHeader = Blob<()>;

impl<B: AsRef<[u8]>> Blob<B> {
    pub fn new(bytes: B) -> Self {
        let len_u16 = u16::try_from(bytes.as_ref().len()).expect("bytes too large");

        Self {
            len: len_u16.into(),
            checksum: 0x42.into(),
            bytes,
        }
    }
}

impl<B> From<Blob<B>> for Packet<Blob<B>> {
    fn from(blob: Blob<B>) -> Self {
        Self {
            discriminant: PacketDiscriminant::Blob,
            payload: blob,
        }
    }
}

impl<B> Blob<B> {
    fn header(&self) -> BlobHeader {
        BlobHeader {
            len: self.len,
            checksum: self.checksum,
            bytes: (),
        }
    }
}

impl<B: Persist> Persist for Blob<B> {
    #[inline]
    fn write_canonical<W: Write>(&self, dst: W) -> io::Result<W> {
        let dst = self.len.write_canonical(dst)?;
        let dst = self.checksum.write_canonical(dst)?;
        let dst = self.bytes.write_canonical(dst)?;
        Ok(dst)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ValidateBlobError {
    expected_checksum: u16,
    actual_checksum: u16,
}

impl Blob<[u8]> {
    pub fn validate(buf: &[u8]) -> Result<(&Self, &[u8]), Error<ValidateBlobError>> {
        let (header, rest) = try_split_at(buf, mem::size_of::<BlobHeader>())?;

        // Safe because all our fields are unconditionally valid
        let header: &BlobHeader = unsafe { &*(header.as_ptr() as *const _) };

        let (_bytes, rest) = try_split_at(rest, header.len.get() as usize)?;

        let this: &Self = unsafe {
            mem::transmute::<&[u8], &Blob>(
                slice::from_raw_parts(buf.as_ptr(),
                                      buf.len() - mem::size_of::<BlobHeader>())
            )
        };

        // FIXME: calculate checksum for real

        Ok((this, rest))
    }
}

pub fn calc_padding_words_required(initial_offset: u64, header: &[u8], mut payload: &[u8]) -> usize {
    let (prefix, header_words, remainder) = unsafe { header.align_to::<Word>() };
    assert_eq!(prefix.len(), 0);

    let mut mid_buf = [0; mem::size_of::<Word>()];
    let mid: &[Word] = if remainder.len() > 0 {
        let needed = mem::size_of::<Word>() - remainder.len();
        let mid = cmp::min(needed, remainder.len());
        let (got, new_payload) = payload.split_at(mid);
        payload = new_payload;

        mid_buf[0 .. remainder.len()].copy_from_slice(remainder);
        mid_buf[remainder.len() .. remainder.len() + mid].copy_from_slice(got);

        let (_, mid, _) = unsafe { mid_buf.align_to::<Word>() };
        mid
    } else {
        &[]
    };

    let (prefix, payload_words, remainder) = unsafe { payload.align_to::<Word>() };
    assert_eq!(prefix.len(), 0);

    let mut last_buf = [0; mem::size_of::<Word>()];
    let last: &[Word] = if remainder.len() > 0 {
        last_buf[0 .. remainder.len()].copy_from_slice(remainder);

        let (_, last, _) = unsafe { last_buf.align_to::<Word>() };
        last
    } else {
        &[]
    };

    let all_words = header_words.iter()
                                .chain(mid)
                                .chain(payload_words)
                                .chain(last);

    let mut padding_len = 0;
    'outer: loop {
        for (i, word) in all_words.clone().enumerate() {
            let offset = initial_offset + (padding_len as u64) + (i as u64);
            let mark = u64::max_value() - offset;

            // We have a conflict. Try more padding.
            if word.get() == mark {
                padding_len += 1;
                continue 'outer;
            }
        }

        break padding_len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calc_padding_words_required() {
        macro_rules! t {
            ($initial_offset:expr, $header:expr, $payload:expr => $padding_required:expr) => {{
                let padding_required = $padding_required;
                assert_eq!(calc_padding_words_required($initial_offset, $header, $payload), padding_required);
            }}
        }

        t!(1, &[], &[] => 0);

        t!(u64::max_value() - (0xfe + 1),
           &[0xfe,0xff,0xff,0xff,0xff,0xff,0xff,0xff],
           &[0xfe]
           => 1);

        t!(u64::max_value() - (0xfe + 1),
           &[0xfe,0xff,0xff,0xff,0xff,0xff,0xff,0xff],
           &[0xfe]
           => 1);

        t!(u64::max_value() - (0xfe + 1),
           &[0xfe,0xff,0xff,0xff,0xff,0xff,0xff,0xff],
           &[0xfe,0x00]
           => 1);

        t!(u64::max_value() - (0xfe + 1),
           &[0xfe,0xff,0xff,0xff,0xff,0xff,0xff,0xff],
           &[0xfe,0x00]
           => 1);

        t!(u64::max_value() - (0xfe + 1),
           &[0xfe,0xff,0xff,0xff,0xff,0xff,0xff,0xff],
           &[0xfe,0x00,0x00,0x00,0x00,0x00,0x00,0x00]
           => 1);
    }

    /*
    #[test]
    fn test_calc_padding_words_required() {
        macro_rules! t {
            ($initial_offset:expr, $words:expr, $padding_required:expr) => {{
                assert_eq!(calc_padding_words_required($initial_offset, $words), $padding_required);
            }}
        }

        t!(1, &[], 0);
        t!(1, &[u64::max_value(), 0, 0], 0);
        t!(1, &[u64::max_value() - 1, 0, 0], 1);
    }
    */
}

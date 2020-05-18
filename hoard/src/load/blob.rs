use std::any::type_name;
use std::borrow::Borrow;
use std::error::Error;
use std::fmt;
use std::marker::PhantomData;
use std::mem;

use thiserror::Error;

use super::*;

pub trait BlobLoader<'a> : Sized {
    type Done;
    type Zone;

    fn zone(&self) -> &Self::Zone;

    fn load_bytes<F, R>(&mut self, size: usize, f: F) -> R
        where F: FnOnce(&[u8]) -> R;

    fn decode<U>(&mut self) -> Result<U, U::Error>
        where U: Decode<Self::Zone>;

    fn done(self) -> Self::Done;
}

pub trait BlobDecoder : Sized {
    type Done;
    type Zone;

    fn zone(&self) -> &Self::Zone;

    fn decode_bytes<F, R>(&mut self, size: usize, f: F) -> R
        where F: FnOnce(&[u8]) -> R;

    fn decode<U>(&mut self) -> Result<U, U::Error>
        where U: Decode<Self::Zone>;

    fn done(self) -> Self::Done;
}

#[derive(Debug)]
pub struct SliceLoader<'a, Z> {
    zone: &'a Z,
    remaining: &'a [u8],
}

impl<'a, Z> SliceLoader<'a, Z> {
    pub fn new(slice: &'a [u8], zone: &'a Z) -> Self {
        Self {
            zone,
            remaining: slice,
        }
    }
}

impl<'a, Z> BlobLoader<'a> for SliceLoader<'a, Z> {
    type Done = ();
    type Zone = Z;

    fn zone(&self) -> &Z {
        self.zone
    }

    fn load_bytes<F, R>(&mut self, size: usize, f: F) -> R
        where F: FnOnce(&[u8]) -> R
    {
        assert!(self.remaining.len() >= size);
        let (buf, remaining) = self.remaining.split_at(size);
        self.remaining = remaining;

        f(buf)
    }

    fn decode<F>(&mut self) -> Result<F, F::Error>
        where F: Decode<Self::Zone>
    {
        assert!(self.remaining.len() >= F::BLOB_LEN);

        let zone = self.zone;
        self.decode_bytes(F::BLOB_LEN, |buf| {
            F::decode_blob(SliceLoader::new(buf, zone))
                .map(|(_, u)| u)
        })
    }

    fn done(self) -> Self::Done {
        assert_eq!(self.remaining.len(), 0);
    }
}

impl<'a, Z> BlobDecoder for SliceLoader<'a, Z> {
    type Done = ();
    type Zone = Z;

    fn zone(&self) -> &Z {
        self.zone
    }

    fn decode_bytes<F, R>(&mut self, size: usize, f: F) -> R
        where F: FnOnce(&[u8]) -> R
    {
        assert!(self.remaining.len() >= size);
        let (buf, remaining) = self.remaining.split_at(size);
        self.remaining = remaining;

        f(buf)
    }

    fn decode<F>(&mut self) -> Result<F, F::Error>
        where F: Decode<Self::Zone>
    {
        assert!(self.remaining.len() >= F::BLOB_LEN);

        let zone = self.zone;
        self.decode_bytes(F::BLOB_LEN, |buf| {
            F::decode_blob(SliceLoader::new(buf, zone))
                .map(|(_, u)| u)
        })
    }

    fn done(self) -> Self::Done {
        assert_eq!(self.remaining.len(), 0);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
    }
}

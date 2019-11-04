//! Efficient, append-only, persistence.

use core::borrow::Borrow;
use core::fmt;

use pointee::Pointee;

use crate::refs::Ref;
use crate::{Ptr, Rec};

mod layout;
pub use self::layout::Layout;

mod scalars;
mod tuples;
mod option;

/// Wrapper for pile zones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Pile<P>(P);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NonZeroPadding;

pub trait Zone {
    type PaddingError : fmt::Debug;
}

impl Zone for ! {
    type PaddingError = NonZeroPadding;
}

impl<Z: Zone> super::Zone for Pile<Z> {
    type Ptr = !;
    type Allocator = crate::never::NeverAlloc<Pile<Z>>;
    type Error = !;

    unsafe fn dealloc<T: ?Sized + Pointee>(ptr: Ptr<T,Self>) {
        unimplemented!()
    }
}

/// Fixed-size pile marshalling.
pub trait Marshal<Z: Zone> : Sized {
    /// Error returned when decoding (or verification) fails.
    type Error : fmt::Debug;

    /// The layout of a value of this type when serialized in the given zone.
    const LAYOUT: Layout;

    fn load<L>(loader: L) -> Result<(Self, L::Done), Self::Error>
        where L: Loader<Zone=Z>;

    fn store<D>(self, dumper: D) -> Result<D::Ok, D::Error>
        where D: Dumper<Zone=Z>;
}

pub trait Loader : Sized {
    type Zone : Zone;

    /// The output type produced by a succesful load.
    type Done;

    /// Reads bytes.
    fn read_bytes<B: AsMut<[u8]>>(self, buf: B) -> (B, Self);

    /// Verify that padding bytes are the expected value.
    fn verify_padding(self, len: usize) -> Result<Self, <Self::Zone as Zone>::PaddingError>;

    /// Load a value.
    fn read<T: Marshal<Self::Zone>>(self) -> Result<(T, Self), T::Error>;

    /// Finishes loading.
    fn done(self) -> Self::Done;
}

pub trait Dumper : Sized {
    type Zone : Zone;
    type Ok;
    type Error;

    fn write_bytes(self, buf: &[u8]) -> Result<Self, Self::Error>;
    fn write_padding(self, len: usize) -> Result<Self, Self::Error>;

    fn write<T: Marshal<Self::Zone>>(self, value: T) -> Result<Self, Self::Error>;

    fn finish(self) -> Result<Self::Ok, Self::Error>;
}

impl<T: Marshal<Z>, Z: Zone> super::Load<Pile<Z>> for T {
    type Error = <T as Marshal<Z>>::Error;
    type Owned = T;

    fn load<'p>(zone: &Pile<Z>, r: &'p super::Rec<Self,Pile<Z>>) -> Result<Ref<'p, Self, Pile<Z>>, Self::Error> {
        unimplemented!()
    }
}

impl Loader for &'_ [u8] {
    type Zone = !;
    type Done = ();

    fn read_bytes<B: AsMut<[u8]>>(self, mut buf: B) -> (B, Self) {
        assert!(buf.as_mut().len() <= self.len(),
                "not enough bytes remaining");

        let (src, rest) = self.split_at(buf.as_mut().len());
        buf.as_mut().copy_from_slice(src);
        (buf, rest)
    }

    fn verify_padding(self, len: usize) -> Result<Self, <Self::Zone as Zone>::PaddingError> {
        assert!(len <= self.len(),
                "not enough bytes remaining");

        let (padding, rest) = self.split_at(len);
        if padding.iter().all(|x| *x == 0) {
            Ok(rest)
        } else {
            Err(NonZeroPadding)
        }
    }

    fn read<T: Marshal<Self::Zone>>(self) -> Result<(T, Self), T::Error> {
        assert!(T::LAYOUT.len() <= self.len(),
                "not enough bytes remaining");
        let (b, rest) = self.split_at(T::LAYOUT.len());
        let (v, ()) = T::load(b)?;
        Ok((v, rest))
    }

    fn done(self) -> Self::Done {
        assert_eq!(self.len(), 0,
                   "Not all bytes used; {} remaining", self.len());
        ()
    }
}

impl Dumper for Vec<u8> {
    type Zone = !;
    type Ok = Self;
    type Error = !;

    fn write_bytes(mut self, buf: &[u8]) -> Result<Self, Self::Error> {
        self.extend_from_slice(buf);
        Ok(self)
    }
    fn write_padding(mut self, len: usize) -> Result<Self, Self::Error> {
        self.resize_with(self.len() + len, u8::default);
        Ok(self)
    }

    fn write<T: Marshal<Self::Zone>>(self, value: T) -> Result<Self, Self::Error> {
        let orig_len = self.len();
        let this = value.store(self)?;
        assert_eq!(this.len() - orig_len, T::LAYOUT.len(),
                   "more bytes written than expected");
        Ok(this)
    }

    fn finish(self) -> Result<Self::Ok, Self::Error> {
        Ok(self)
    }
}

impl<Z: Zone> Marshal<Z> for Pile<Z> {
    type Error = !;
    const LAYOUT: Layout = Layout::new(0);

    fn load<L>(loader: L) -> Result<(Self, L::Done), Self::Error>
        where L: Loader<Zone=Z>
    {
        unimplemented!()
    }

    fn store<D: Dumper<Zone=Z>>(self, dumper: D) -> Result<D::Ok, D::Error> {
        dumper.finish()
    }
}


#[derive(Debug)]
pub struct LoadRecError;

impl<T: ?Sized + Pointee, Z: Zone> Marshal<Z> for Rec<T, Pile<Z>>
where T::Metadata: Marshal<!>,
{
    type Error = LoadRecError;

    const LAYOUT: Layout = T::Metadata::LAYOUT;

    fn load<L>(loader: L) -> Result<(Self, L::Done), Self::Error>
        where L: Loader<Zone=Z>
    {
        unimplemented!()
    }

    fn store<D>(self, dumper: D) -> Result<D::Ok, D::Error>
        where D: Dumper<Zone=Z>
    {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
    }
}

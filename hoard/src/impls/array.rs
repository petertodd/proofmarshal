use core::fmt;
use core::mem::{self, MaybeUninit};

use thiserror::Error;

use sliceinit::SliceInitializer;

use super::*;


impl<T: Load, const N: usize> Load for [T;N] {
    type Error = LoadArrayError<T::Error, N>;

    fn load<'a>(mut blob: BlobCursor<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        for i in 0 .. N {
            blob.field::<T>().map_err(|err| LoadArrayError { idx: i, err })?;
        }

        unsafe { Ok(blob.assume_valid()) }
    }
}

#[derive(Error, Debug, PartialEq, Eq)]
//#[error("array validation failed at index {idx}: {err}")]
#[error("array validation failed")]
pub struct LoadArrayError<E: fmt::Debug, const N: usize> {
    idx: usize,
    err: E,
}

impl<E: fmt::Debug + Into<!>, const N: usize> From<LoadArrayError<E, N>> for ! {
    fn from(err: LoadArrayError<E,N>) -> ! {
        err.err.into()
    }
}

impl<'a, P, T: Save<P>, const N: usize> Save<P> for [T; N] {
    type State = [T::State; N];

    fn init_save_state(&self) -> Self::State {
        let mut r: [MaybeUninit<T::State>; N] = unsafe { MaybeUninit::uninit().assume_init() };
        let mut initializer = SliceInitializer::new(&mut r[..]);

        for item in self.iter() {
            initializer.push(item.init_save_state())
        }

        initializer.done();

        // Need a transmute_copy() as Rust doesn't seem to know the two arrays are the same size.
        let r2 = unsafe { mem::transmute_copy(&r) };
        assert_eq!(mem::size_of_val(&r), mem::size_of_val(&r2));
        assert_eq!(mem::align_of_val(&r), mem::align_of_val(&r2));

        r2
    }

    unsafe fn poll<D: SavePtr<P>>(&self, state: &mut Self::State, mut dumper: D) -> Result<D, D::Error> {
        for (item, state) in self.iter().zip(state.iter_mut()) {
            dumper = item.poll(state, dumper)?;
        }
        Ok(dumper)
    }

    unsafe fn encode<W: WriteBlob>(&self, state: &Self::State, mut dst: W) -> Result<W::Ok, W::Error> {
        for (item, state) in self.iter().zip(state.iter()) {
            dst = dst.write(item, state)?;
        }
        dst.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use core::convert::TryFrom;

    #[test]
    fn test() {
        /*
        let bytes = Bytes::<[u8;0]>::try_from(&[][..]).unwrap();
        let blob = Blob::from(&bytes).into_cursor();
        Validate::validate(blob).unwrap();

        let bytes = Bytes::<[u8;10]>::try_from(&[0,1,2,3,4,5,6,7,8,9][..]).unwrap();
        let blob = Blob::from(&bytes).into_cursor();
        let valid = Validate::validate(blob).unwrap();
        assert_eq!(valid.to_ref(), &[0,1,2,3,4,5,6,7,8,9]);
        */
    }
}

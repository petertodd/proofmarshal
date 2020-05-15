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

/*
pub struct ValidateArrayState<S> {
    idx: usize,
    state: S,
}

impl<'a, P, T: Validate<'a, P>, const N: usize> Validate<'a, P> for [T; N] {
    type State = ValidateArrayState<T::State>;

    fn init_validate_state(&self) -> Self::State {
        todo!()
    }

    fn poll<V: ValidatePtr<P>>(&'a self, state: &mut Self::State, validator: &mut V) -> Result<(), V::Error> {
        todo!()
    }
}
*/

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

impl<'a, R, T, const N: usize> Saved<R> for [T; N]
where T: Saved<R>,
      T::Saved: Sized,
{
    type Saved = [T::Saved; N];
}

impl<'a, Q, R, T, const N: usize> Save<'a, Q, R> for [T; N]
where T: Save<'a, Q, R>,
      T::Saved: Sized,
{
    type State = [T::State; N];

    fn init_save_state(&'a self) -> Self::State {
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

    fn save_poll<D: SavePtr<Q, R>>(&'a self, state: &mut Self::State, mut dumper: D) -> Result<D, D::Error> {
        for (item, state) in self.iter().zip(state.iter_mut()) {
            dumper = item.save_poll(state, dumper)?;
        }
        Ok(dumper)
    }

    fn save_blob<W: SaveBlob>(&'a self, state: &Self::State, dst: W) -> Result<W::Done, W::Error> {
        let dst = dst.alloc(mem::size_of::<Self::Saved>())?;
        self.encode_blob(state, dst)
    }

    fn encode_blob<W: WriteBlob>(&'a self, state: &Self::State, mut dst: W) -> Result<W::Done, W::Error> {
        for (item, state) in self.iter().zip(state.iter()) {
            dst = dst.write(item, state)?;
        }
        dst.done()
    }
}

impl<T, const N: usize> Primitive for [T; N]
where T: Primitive,
      T::Saved: Sized,
{}

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

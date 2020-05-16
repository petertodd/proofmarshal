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

impl<Q, R, T, const N: usize> Save<Q, R> for [T; N]
where T: Save<Q, R>,
      T::Saved: Sized,
{
    type Thunk = [T::Thunk; N];

    fn save_children<D>(&self, dst: &mut D) -> Self::Thunk
        where D: SavePtr<Source=Q, Target=R>
    {
        let mut r: [MaybeUninit<T::Thunk>; N] = unsafe { MaybeUninit::uninit().assume_init() };
        let mut initializer = SliceInitializer::new(&mut r[..]);

        for item in self.iter() {
            initializer.push(item.save_children(dst))
        }

        initializer.done();

        // Need a transmute_copy() as Rust doesn't seem to know the two arrays are the same size.
        let r2 = unsafe { mem::transmute_copy(&r) };
        assert_eq!(mem::size_of_val(&r), mem::size_of_val(&r2));
        assert_eq!(mem::align_of_val(&r), mem::align_of_val(&r2));

        r2
    }
}

impl<Q, R, T, const N: usize> SavePoll<Q, R> for [T; N]
where T: SavePoll<Q, R>,
      T::Target: Sized,
{
    type Target = [T::Target; N];

    fn save_poll<D>(&mut self, mut dst: D) -> Result<D, D::Error>
        where D: SavePtr<Source=Q, Target=R>
    {
        for thunk in self.iter_mut() {
            dst = thunk.save_poll(dst)?;
        }
        Ok(dst)
    }

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
        for thunk in self.iter() {
            //dst = dst.write(thunk)?;
            todo!()
        }
        dst.done()
    }

    fn save_blob<W: SaveBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
        let dst = dst.alloc(mem::size_of::<Self::Target>())?;
        self.encode_blob(dst)
    }
}

/*
impl<T, const N: usize> Primitive for [T; N]
where T: Primitive,
      T::Saved: Sized,
{}
*/

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

use core::mem::{self, MaybeUninit};

use sliceinit::SliceInitializer;

use super::*;

use crate::blob::StructValidator;

#[derive(Debug, PartialEq, Eq)]
pub struct ValidateArrayError<E, const N: usize> {
    idx: usize,
    err: E,
}

impl<E: ValidationError, const N: usize> ValidationError for ValidateArrayError<E, N> {
}

impl<T: Validate, const N: usize> Validate for [T; N] {
    type Error = ValidateArrayError<T::Error, N>;

    fn validate<B: BlobValidator<Self>>(blob: B) -> Result<B::Ok, B::Error> {
        let mut blob = blob.validate_struct();

        for i in 0 .. N {
            blob.field::<T,_>(|err| ValidateArrayError { idx: i, err })?;
        }

        unsafe { blob.assume_valid() }
    }
}

unsafe impl<Z: Zone, T: Load<Z>, const N: usize> Load<Z> for [T; N] {
    type ValidateChildren = [T::ValidateChildren; N];

    fn validate_children(&self) -> [T::ValidateChildren; N] {
        let mut r: [MaybeUninit<T::ValidateChildren>; N] = unsafe { MaybeUninit::uninit().assume_init() };

        let mut initializer = SliceInitializer::new(&mut r[..]);

        for item in self.iter() {
            initializer.push(item.validate_children())
        }

        initializer.done();

        // Need a transmute_copy() as Rust doesn't seem to know the two arrays are the same size.
        let r2 = unsafe { mem::transmute_copy(&r) };
        assert_eq!(mem::size_of_val(&r), mem::size_of_val(&r2));
        assert_eq!(mem::align_of_val(&r), mem::align_of_val(&r2));

        r2
    }
}

impl<Z: Zone, T: ValidateChildren<Z>, const N: usize> ValidateChildren<Z> for [T; N] {
    fn poll<V: PtrValidator<Z>>(&mut self, ptr_validator: &V) -> Result<(), V::Error> {
        for item in self.iter_mut() {
            item.poll(ptr_validator)?;
        }
        Ok(())
    }
}

/*
use crate::pile::Pile;
use crate::zone::validptr::{ValidPtr, ValidateState};
pub fn test_validate_children<'p,'v>(array: &[ValidPtr<ValidPtr<bool, Pile<'p,'v>>, Pile<'p,'v>>; 250])
-> [ValidateState<ValidPtr<bool, Pile<'p,'v>>, Pile<'p,'v>>; 250]
{
    <_ as Load<Pile<'p,'v>>>::validate_children(array)
}

use crate::blob::{Blob, ValidBlob};

pub fn test_validate_ptr100<'a,'p,'v>(blob: Blob<'a, [ValidPtr<bool, Pile<'p,'v>>;100]>)
-> Result<ValidBlob<'a,[ValidPtr<bool, Pile<'p,'v>>;100]>,
          crate::blob::Error<ValidateArrayError<
              crate::zone::fatptr::ValidateError<!, crate::pile::offset::ValidateOffsetError>, 100>>>
{
    <_ as Validate>::validate(blob.into_validator())
}

pub fn test_validate_ptr<'a,'p,'v>(blob: Blob<'a, ValidPtr<bool, Pile<'p,'v>>>)
-> Result<ValidBlob<'a,ValidPtr<bool, Pile<'p,'v>>>,
          crate::blob::Error<
              crate::zone::fatptr::ValidateError<!, crate::pile::offset::ValidateOffsetError>>>
{
    <_ as Validate>::validate(blob.into_validator())
}

pub fn test_validate100(blob: Blob<[bool;100]>)
-> Result<ValidBlob<[bool;100]>, crate::blob::Error<ValidateArrayError<crate::load::impls::BoolError, 100>>>
{
    <_ as Validate>::validate(blob.into_validator())
}

pub fn test_validate1(blob: Blob<[bool;1]>)
-> Result<ValidBlob<[bool;1]>, crate::blob::Error<ValidateArrayError<crate::load::impls::BoolError, 1>>>
{
    <_ as Validate>::validate(blob.into_validator())
}

pub fn test_validate_infallible(blob: Blob<[u8;1]>)
-> Result<ValidBlob<[u8;1]>, crate::blob::Error<ValidateArrayError<!, 1>>>
{
    Validate::validate(blob.into_validator())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let blob = Blob::<[bool;1]>::new(&[1], ()).unwrap();

        test_validate1(blob).unwrap();

        //let blob = Validate::validate(blob.into_validator()).unwrap();
    }
}
*/

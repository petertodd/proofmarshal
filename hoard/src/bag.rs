use super::*;

use core::fmt;

use crate::marshal::blob::*;
use crate::marshal::*;

/// An owned pointer to a value in a `Zone`.
#[derive(Debug)]
pub struct Bag<T: ?Sized + Pointee, Z: Zone> {
    ptr: OwnedPtr<T,Z::Ptr>,
    zone: Z,
}

impl<T: ?Sized + Pointee, Z: Zone> Bag<T,Z> {
    pub fn new(value: impl Take<T>) -> Self
        where Z: Default
    {
        Self::new_in(value, Z::allocator())
    }

    pub fn new_in(value: impl Take<T>, mut alloc: impl Alloc<Zone=Z>) -> Self {
        Self {
            ptr: alloc.alloc(value),
            zone: alloc.zone(),
        }
    }
}

impl<T: ?Sized + Pointee, Z: Zone> Bag<T,Z>
where T: Load<Z>, Z: Get
{
    pub fn get(&self) -> Ref<T> {
        self.zone.get(&self.ptr)
    }

    pub fn take(self) -> T::Owned {
        self.zone.take(self.ptr)
    }
}

unsafe impl<T, Z, Y> Encode<Y> for Bag<T, Z>
where Y: BlobZone,
      Z: Zone + Encode<Y>,
      T: ?Sized + Save<Y>,
{
    type State = (); //<OwnedPtr<T, Z::Ptr> as Encode<Y>>::State;

    fn blob_layout() -> BlobLayout {
        //<OwnedPtr<T, Z::Ptr> as Encode<Y>>::blob_layout()
        todo!()
    }

    fn init_encode_state(&self) -> Self::State {
        //self.ptr.init_encode_state()
        todo!()
    }

    fn encode_poll<D: SavePtr<Y>>(&self, state: &mut Self::State, dumper: D) -> Result<D, D::Pending> {
        //<OwnedPtr<T, Z::Ptr> as Encode<Y>>::encode_poll(&self.ptr, state, dumper)
        todo!()
    }

    fn encode_blob<W: WriteBlob>(&self, state: &Self::State, dst: W) -> Result<W::Ok, W::Error> {
        //<OwnedPtr<T, Z::Ptr> as Encode<Y>>::encode_blob(&self.ptr, state, dst)
        todo!()
    }
}

impl<T, Z, Y> Decode<Y> for Bag<T, Z>
where Y: BlobZone,
      Z: Zone + Decode<Y>,
      T: ?Sized + Load<Y>,
{
    type Error = !;
    type ValidateChildren = ();

    fn validate_blob<'a>(blob: Blob<'a, Self, Y>) -> Result<BlobValidator<'a, Self, Y>, Self::Error> {
        /*
        let mut fields = blob.validate_struct();
        let ptr_state = fields.field::<OwnedPtr<T, Z::Ptr>>()?;
        Ok(fields.done(ptr_state))
        */
        todo!()
    }

    fn decode_blob<'a>(blob: FullyValidBlob<'a, Self, Y>, loader: &impl LoadPtr<Y>) -> Self {
        todo!()
    }
}

impl<T: ?Sized + Pointee, Z: Zone> fmt::Pointer for Bag<T,Z>
where Z::Ptr: fmt::Pointer,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Pointer::fmt(&self.ptr, f)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /*
    use crate::pile::PileMut;

    #[test]
    fn test_pile() {
        let bagged_u8 = Bag::<u8, PileMut>::new(42);
        assert_eq!(*bagged_u8.get(), 42);

        let bag2 = Bag::<_, PileMut>::new(bagged_u8);

        bag2.get();
    }
    */
}

/// Persistant binary blob storage.

use core::mem;
use std::io;

use crate::pointee::Pointee;
use crate::persist::{Persist, MaybeValid, Valid};

use crate::ptr::{Ptr, Dealloc};

//pub mod offset;
//pub mod pile;

pub trait BlobArena : super::Arena<Ptr=<Self as BlobArena>::BlobPtr> {
    type BlobPtr : Dealloc + Persist;

    //fn validate_offset<'p, T: ?Sized>(&self, offset: &'p Ptr<T, Self::Offset>) -> Result<&'p Ptr<T, Self::Ptr>, Self::OffsetError>
    //    where T: Persist<Self>;
}

/*
pub trait AllocBlob<A: Arena> {
    fn alloc_bytes(&mut self, len: usize, f: impl FnOnce(&mut [u8])) -> A::Offset;

    fn alloc_blob<T: ?Sized + Persist>(&mut self, value: &T) -> Ptr<T, A::Offset> {
        let offset = self.alloc_bytes(mem::size_of_val(value),
            | dst: &mut [u8] | {
                value.write_canonical_bytes(dst);
            }
        );

        unsafe {
            Ptr::new(offset, value.ptr_metadata())
        }
    }
}


unsafe impl<T: ?Sized + Pointee, P: Dealloc, A> Persist<A> for Ptr<T, P>
where A: Arena<Offset=P>,
      P: Persist,
{
    type Error = !;

    fn validate_bytes<'a>(unver: MaybeValid<'a, Self, [u8]>, arena: &A) -> Result<Valid<'a, Self, [u8]>, Self::Error> {
        unimplemented!()
    }
}
*/

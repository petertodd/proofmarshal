//! Saving/encoding of data to zones.

use std::any::Any;
use std::error;
use std::mem;

use crate::pointee::Pointee;
use crate::blob::*;
use crate::zone::*;
use crate::load::*;
use crate::scalar::Scalar;

use super::*;

/// A `Sized` value that can be saved persistently in a zone.
pub trait Encode<Y: Zone> : Decode {
    /// The resulting type when this value is encoded for the target zone.
    type Encoded : BlobSize;

    type EncodePoll : EncodePoll<SrcZone = Self::Zone, SrcPtr = Self::Ptr, DstZone = Y, Target = Self::Encoded>;

    fn init_encode(&self) -> Self::EncodePoll;
}

pub trait EncodePoll {
    type SrcZone : Zone;
    type SrcPtr : Ptr;
    type DstZone : Zone;
    type Target : BlobSize;

    fn encode_poll<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: Saver<DstZone = Self::DstZone>;
}

pub trait Save<Y: Zone> : Load {
    /// The resultant type when this value is saved in the target zone.
    type Saved : ?Sized + Pointee<Metadata = Self::Metadata>;
}

impl<Y: Zone, T: Encode<Y>> Save<Y> for T {
    type Saved = T::Encoded;
}

/// Saves data in one zone to another zone.
pub trait Saver {
    type SrcZone : Zone;

    /// The zone this `Saver` saves data into.
    type DstZone : Zone;

    /// The error returned when an operation fails.
    type Error;

    /*
    /// Tries to save clean data.
    ///
    /// In the typical case of saving dirty data to the same zone, the `SrcPtr` will be the same as
    /// the `DstZone`'s persistent pointer, and thus this function will be a simple no-op.
    fn save_clean_ptr<T>(&self, ptr: &impl AsPtr<Self::SrcPtr>, metadata: T::Metadata)
        -> Result<Result<<Self::DstZone as Zone>::PersistPtr, &T>,
                  Self::Error>
        where T: ?Sized + Pointee;
    */

    /*
    /// Saves a value whose children have been saved.
    fn save<T>(&mut self, poll: &mut T) -> Result<<Self::DstZone as Zone>::PersistPtr, Self::Error>
        where T: SavePoll<Self::DstZone, Self::SrcPtr>,
              Self::DstZone: Zone;
    */
}

/*
pub trait Save<Y: Zone> : Load {
    type SrcPtr : PersistPtr;
    type Saved : ?Sized + Load + Pointee<Metadata = Self::Metadata>;
    type SavePoll : SavePoll<Y, SrcPtr = Self::SrcPtr, Target = Self::Saved>;

    fn init_save(&self) -> Self::SavePoll;
}

pub trait SavePoll<Y: Zone> {
    type SrcPtr : PersistPtr;
    type Target : ?Sized + Load;

    /// Returns the metadata for the target value.
    ///
    /// The provided implementation works for any type whose metadata is `()`.
    fn target_metadata(&self) -> <Self::Target as Pointee>::Metadata {
        let unit: &dyn Any = &();
        if let Some(metadata) = unit.downcast_ref::<<Self::Target as Pointee>::Metadata>() {
            *metadata
        } else {
            unimplemented!()
        }
    }

    /// Saves children.
    ///
    /// The provided implementation does nothing.
    fn save_children<D>(&mut self, dst: &mut D) -> Result<(), D::Error>
        where D: Saver<Self::SrcPtr, DstZone = Y>
    {
        let _ = dst;
        Ok(())
    }

    /// Saves a blob.
    fn save_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Ok, W::Error>;
}


*/

pub trait WriteBlob : Sized {
    type Ok;
    type Error;

    fn write_bytes(self, buf: &[u8]) -> Result<Self, Self::Error>;

    fn write_padding(self, len: usize) -> Result<Self, Self::Error> {
        todo!()
    }

    //fn write<Y: Zone, T: SavePoll<Y>>(self, value: &T) -> Result<Self, Self::Error>;
    fn finish(self) -> Result<Self::Ok, Self::Error>;
}

impl WriteBlob for Vec<u8> {
    type Error = !;
    type Ok = Self;

    fn write_bytes(self, buf: &[u8]) -> Result<Self, Self::Error> {
        todo!()
    }

    /*
    fn write<Y: Zone, T: SavePoll<Y>>(self, value: &T) -> Result<Self, Self::Error> {
        todo!()
    }
    */

    fn finish(self) -> Result<Self, !> {
        Ok(self)
    }
}

/*

/*
pub trait Encoded<Y: Zone> : Sized {
    type Encoded : Decode<Y>;
}
*/

/*
impl<Y: Zone, T: Encoded<Y>> Saved<Y> for T {
    type Saved = T::Encoded;
}

pub trait SavePoll<Y: Zone, Q: Ptr> {
    type Target : ?Sized + Load<Y>;



}
*/

/*
/// A **type** that can be saved to a blob behind a pointer.
pub trait Save<Y: Zone, Q: Ptr> : Saved<Y> {
    type SavePoll : SavePoll<Y, Q, Target = Self::Saved>;

    fn init_save<D>(&self) -> Self::SavePoll;
}

pub trait SavePoll<Y: Zone, Q: Ptr> {

    fn save_children<D>(&mut self, dst: &mut D) -> Result<(), D::Error>
        where D: Saver<DstZone = Y, SrcPtr = Q>;


}

pub trait EncodePoll<Y: Zone, Q: Ptr> {
    type Target : Decode<Y>;

    fn save_children<D>(&mut self, dst: &mut D) -> Result<(), D::Error>
        where D: Saver<DstZone = Y, SrcPtr = Q>;

    fn encode_blob<W: WriteBlob>(&self, dst: &mut W) -> Result<(), W::Error>;
}
*/

/*
pub trait Encode<Y: Zone, Q: Ptr> : Encoded<Y> {
    type EncodePoll : SaveChildren<Y,Q> + EncodeBlob<Y, Target = Self::Encoded>;

    fn init_encode(&self) -> Self::EncodePoll;
}

pub trait EncodeBlob<Y: Zone> {
    type Target : Decode<Y>;

    fn encode_blob<W: WriteBlob<Zone = Y>>(&self, dst: W) -> Result<W::Done, W::Error>;
}
*/




#[cfg(test)]
mod tests {
    use super::*;

    use crate::zone::Own;
    use crate::blob::*;
    use crate::load::*;
    use crate::zone::*;

    struct Foo<P: Ptr> {
        owned: Own<u8, P, ()>,
        prim: u8,
    }

    impl<P: Ptr> BlobSize for Foo<P> {
        const BLOB_LAYOUT: BlobLayout = <Own<u8, P, ()> as BlobSize>::BLOB_LAYOUT.extend(<u8 as BlobSize>::BLOB_LAYOUT);
    }

    impl<V: Copy, P: Ptr> ValidateBlob<V> for Foo<P>
        where P: BlobSize + ValidateBlob<V>
    {
        type Error = !;

        fn validate_blob<'a>(blob: Blob<'a, Self>, padval: V) -> Result<ValidBlob<'a, Self>, Self::Error> {
            let mut fields = blob.validate_fields(padval);
            fields.field::<Own<u8, P, ()>>().unwrap();
            fields.field::<u8>().unwrap();
            unsafe { Ok(fields.finish()) }
        }
    }

    impl<Z, P: Ptr + Decode<Z>> Load<Z> for Foo<P> {
        fn decode_blob(blob: ValidBlob<Self>, zone: &Z) -> Self::Owned
            where Z: BlobZone
        {
            todo!()
        }
    }

    impl<Y: Zone, P: Ptr> Saved<Y> for Foo<P> {
        type Saved = Foo<Y::Ptr>;
    }

    struct FooSavePoll<Y: Zone, Q: Ptr, P: Ptr + AsPtr<Q>> {
        owned: <Own<u8, P, ()> as Save<Y, Q>>::SavePoll,
        prim: <u8 as Save<Y, Q>>::SavePoll,
    }

    impl<Y: Zone, Q: Ptr, P: Ptr> SavePoll<Y, Q> for FooSavePoll<Y, Q, P>
        where P: AsPtr<Q>,
    {
        type Target = Foo<Q>;

        fn save_blob<W: WriteBlob<Y, Q>>(&self, dst: W) -> Result<W::Done, W::Error>
            where Y: BlobZone
        {
            todo!()
        }
    }
}


/*
pub trait Encode<Y, P = <Y as crate::zone::Zone>::Ptr> : Sized {
    type Encoded : Decode<Y>;
    type EncodePoll : EncodePoll<Y, P, Target = Self::Encoded>;

    fn init_encode_blob<D>(&self, dst: &D) -> Result<Self::EncodeBlobPoll, D::Error>
        where D: BlobSaver<DstZone = Y, SrcPtr = P>;
}

/*
impl<Y, P, T> Save<Y, P> for T
where T: Encode<Y, P>,
      T::Encoded: Pointee<Metadata = <T as Pointee>::Metadata>
{
    type Saved = T::Encoded;
    type SavePoll = T::EncodePoll;

    fn init_save<D>(&self, dst: &D) -> Result<Self::SavePoll, D::Error>
        where D: Saver<Y, P>
    {
        self.init_encode(dst)
    }
}
*/

pub trait EncodeBlobPoll<Y, P = <Y as crate::zone::Zone>::Ptr> {
    type Target : Decode<Y>;

    fn encode_blob_poll<D>(&mut self, dst: &mut D) -> Result<(), D::Error>
        where D: BlobSaver<DstZone = Y, SrcPtr = P>;

    fn encode_blob<W: Write>(&self, dst: &mut W) -> Result<(), W::Error>;
}

/*
impl<Y, P, T> SavePoll<Y, P> for T
where T: EncodePoll<Y, P>
{
    type Target = T::Target;

    fn save_poll<D>(&mut self, dst: &mut D) -> Result<Y::BlobPtr, D::Error>
        where D: Saver<Y, P>,
              Y: BlobZone,
    {
        self.encode_poll(dst)?;

        dst.alloc_blob(T::Target::BLOB_LAYOUT.size(), |dst| {
            self.encode(dst)
        })
    }
}
*/

pub trait AllocBlob {
    type Error;
    type Done;

    type WriteBlob : WriteBlob<Done = Self::Done, Error = Self::WriteBlobError>;
    type WriteBlobError : Into<Self::Error>;

    fn alloc_blob(self, size: usize) -> Result<Self::WriteBlob, Self::Error>;
}

pub trait WriteBlob : Write {
    type Done;

    fn finish(self) -> Result<Self::Done, Self::Error>;
}

pub trait BlobSaver : Sized {
    type DstZone;
    type SrcPtr;

    type Error;

    type Write : Write;

    unsafe fn try_get_dirty<T>(&self, ptr: &Self::SrcPtr, metadata: T::Metadata)
        -> Result<Result<&T, <Self::DstZone as BlobZone>::BlobPtr>,
                  Self::Error>
        where T: ?Sized + Pointee,
              Self::DstZone: BlobZone;

    fn raise_layout_err(&self, err: impl std::error::Error) -> Self::Error {
        todo!()
    }

    fn alloc_blob<F>(&mut self, size: usize, f: F) -> Result<<Self::DstZone as BlobZone>::BlobPtr, Self::Error>
        where F: FnOnce(&mut Self::Write) -> Result<(), <Self::Write as Write>::Error>,
              Self::DstZone: BlobZone;
}

/// Macro to implement `blob::Encode` for a primitive type.
#[macro_export]
macro_rules! impl_encode_blob_for_primitive {
    (| $this:ident : $t:ty, $dst:ident | $save_expr:expr ) => {
        impl<__Y, __P> $crate::blob::EncodeBlob<__Y, __P> for $t {
            type Encoded = Self;
            type EncodeBlobPoll = Self;

            fn init_encode_blob<__D>(&self, __dst: &__D) -> Result<Self::EncodeBlobPoll, __D::Error>
                where __D: $crate::blob::BlobSaver
            {
                Ok(self.clone())
            }
        }

        impl<__Y, __P> $crate::blob::EncodeBlobPoll<__Y, __P> for $t {
            type Target = Self;

            fn encode_blob_poll<__D>(&mut self, __dst: &mut __D) -> Result<(), __D::Error>
                where __D: $crate::blob::BlobSaver
            {
                Ok(())
            }

            fn encode_blob<__W>(&self, $dst: &mut __W) -> Result<(), __W::Error>
                where __W: $crate::blob::Write
            {
                let $this = self;
                $save_expr
            }
        }
    }
}
*/
*/

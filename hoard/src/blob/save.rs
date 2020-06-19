use std::error;
use std::mem;

use crate::pointee::Pointee;
use crate::zone::{Ptr};
use crate::load::{Load, Decode};

use super::*;

pub use crate::writebytes::Write;

/// A **type** that can be saved to a blob behind a pointer.
pub trait SaveBlob<Y, P = <Y as crate::zone::Zone>::Ptr> : Pointee {
    type Saved : ?Sized + Pointee<Metadata = Self::Metadata> + Load<Y>;
    type SaveBlobPoll : SaveBlobPoll<Y, P, Target = Self::Saved>;

    fn init_save_blob<D>(&self, dst: &D) -> Result<Self::SaveBlobPoll, D::Error>
        where D: BlobSaver<DstZone = Y, SrcPtr = P>;
}

/// The asyncronous implementation of `SaveBlob`.
pub trait SaveBlobPoll<Y, P = <Y as crate::zone::Zone>::Ptr> {
    type Target : ?Sized + Load<Y>;

    fn save_blob_poll<D>(&mut self, dst: &mut D) -> Result<(), D::Error>
        where D: BlobSaver<DstZone = Y, SrcPtr = P>;

    fn save_blob<W: AllocBlob>(&self, dst: W) -> Result<W::Done, W::Error>;
}

pub trait EncodeBlob<Y, P = <Y as crate::zone::Zone>::Ptr> : Sized {
    type Encoded : Decode<Y>;
    type EncodeBlobPoll : EncodeBlobPoll<Y, P, Target = Self::Encoded>;

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

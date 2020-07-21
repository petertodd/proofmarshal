//! Saving/encoding of data to zones.

use std::any::{Any, type_name};
use std::error;
use std::fmt;
use std::marker::PhantomData;
use std::mem;

use crate::pointee::Pointee;
use crate::refs::Ref;
use crate::blob::*;
use crate::ptr::*;
use crate::load::*;
use crate::scalar::Scalar;

use super::*;

/// Provides the projection of a type saved with a specific type of pointer.
pub trait Saved<DstPtr> : Pointee {
    /// The projected type, with all internal pointers replaced by `DstPtr`.
    type Saved : ?Sized + Pointee<Metadata = Self::Metadata>;
}

/// The ability to be saved to a persistent zone using the specified pointer type.
pub trait Save<DstPtr: Ptr> : Saved<DstPtr> + Load {
    type SavePoll : SavePoll<DstPtr = DstPtr, SrcPtr = Self::Ptr, Target = Self::Saved>;

    /// Creates the `SavePoll` state machine to save this value and its children.
    fn init_save(&self) -> Self::SavePoll;
}

pub trait EncodeBlob {
    /// The target value.
    type Target : ?Sized + Pointee;

    /// Gets the metadata for the target value.
    ///
    /// The default implementation works for any type with `()` as its metadata.
    #[inline(always)]
    fn target_metadata(&self) -> <Self::Target as Pointee>::Metadata {
        let unit: &dyn Any = &();
        if let Some(metadata) = unit.downcast_ref::<<Self::Target as Pointee>::Metadata>() {
            *metadata
        } else {
            unimplemented!("{} needs to implement SavePoll::target_metadata()", type_name::<Self>())
        }
    }

    /// Encodes the value once polling is complete.
    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Ok, W::Error>;
}

/// The interface to the state machine that saves a value and its children.
pub trait SavePoll : EncodeBlob {
    /// The type of pointer that children of this value will be behind.
    type SrcPtr : Ptr;

    /// The type of pointer this state machine is saving children to.
    type DstPtr : Ptr;

    /// Polls the state machine, using the provided `Saver`.
    fn save_poll<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: Saver<SrcPtr = Self::SrcPtr, DstPtr = Self::DstPtr>;
}

/// `Save`, but for the specified `SrcPtr`. Implemented automatically.
pub trait SavePtr<SrcPtr: Ptr, DstPtr: Ptr> : Saved<DstPtr> + LoadPtr<SrcPtr> {
    type SavePtrPoll : SavePoll<SrcPtr = SrcPtr, DstPtr = DstPtr, Target = Self::Saved>;
    fn init_save_ptr(&self) -> Self::SavePtrPoll;
}

impl<Q: Ptr, R: Ptr, T: Save<R>> SavePtr<Q, R> for T
where T::Ptr: AsPtr<Q>,
      Q::BlobZone: AsZone<<T::Ptr as Ptr>::BlobZone>,
{
    type SavePtrPoll = SavePtrPoll<Q, R, T>;

    fn init_save_ptr(&self) -> Self::SavePtrPoll {
        SavePtrPoll {
            marker: PhantomData,
            inner: self.init_save(),
        }
    }
}

/// Wrapper used by the automatic `SavePtr` implementation.
#[repr(transparent)]
pub struct SavePtrPoll<Q: Ptr, R: Ptr, T: Save<R>> {
    marker: PhantomData<Q>,
    inner: T::SavePoll,
}

impl<Q: Ptr, R: Ptr, T: Save<R>> fmt::Debug for SavePtrPoll<Q, R, T>
where T::SavePoll: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("inner", &self.inner)
            .finish()
    }
}

impl<Q: Ptr, R: Ptr, T: Save<R>> EncodeBlob for SavePtrPoll<Q, R, T> {
    type Target = T::Saved;

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Ok, W::Error> {
        self.inner.encode_blob(dst)
    }
}

impl<Q: Ptr, R: Ptr, T: Save<R>> SavePoll for SavePtrPoll<Q, R, T>
where T::Ptr: AsPtr<Q>,
      Q::BlobZone: AsZone<<T::Ptr as Ptr>::BlobZone>,
{
    type SrcPtr = Q;

    type DstPtr = R;

    fn save_poll<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: Saver<SrcPtr = Self::SrcPtr, DstPtr = R>,
    {
        // SAFETY: #[repr(transparent)]
        let saver = unsafe { &mut *(saver as *mut S as *mut SaverAdapter<T::Ptr, S>) };
        self.inner.save_poll(saver)
    }
}

#[repr(transparent)]
struct SaverAdapter<Q: Ptr, S: Saver> {
    marker: PhantomData<Q>,
    inner: S,
}

impl<Q: Ptr, S: Saver> Saver for SaverAdapter<Q, S>
where Q: AsPtr<S::SrcPtr>,
      <S::SrcPtr as Ptr>::BlobZone: AsZone<Q::BlobZone>,
{
    type SrcPtr = Q;
    type DstPtr = S::DstPtr;

    type Error = S::Error;

    fn try_save_raw<R, T: ?Sized + ValidateBlob>(&self,
        ptr: &Q::Persist,
        metadata: T::Metadata,
        f: impl FnOnce(ValidBlob<T>, &Q::BlobZone) -> R,
    ) -> Result<Result<<Self::DstPtr as Ptr>::Persist, R>,
                Self::Error>
    {
        self.inner.try_save_raw(
            ptr.as_persist_ptr(),
            metadata,
            |valid_blob, q_blob_zone| {
                f(valid_blob, q_blob_zone.as_zone())
            }
        )
    }

    fn finish_save<T>(&mut self, value_poll: &T) -> Result<<Self::DstPtr as Ptr>::Persist, Self::Error>
        where T: EncodeBlob
    {
        self.inner.finish_save(value_poll)
    }
}

/// Saves data in one zone to another zone.
pub trait Saver {
    type SrcPtr : Ptr;

    type DstPtr : Ptr;

    /// The error returned when an operation fails.
    type Error;

    /// Tries to coerce a source pointer to a destination pointer.
    ///
    /// In the typical case of saving dirty data to the same zone, the `SrcPtr` will be the same as
    /// the `DstZone`'s persistent pointer, and thus this function will be a simple no-op.
    fn try_save_raw<R, T: ?Sized + ValidateBlob>(
        &self,
        ptr: &<Self::SrcPtr as Ptr>::Persist,
        metadata: T::Metadata,
        f: impl FnOnce(ValidBlob<T>, &<Self::SrcPtr as Ptr>::BlobZone) -> R,
    ) -> Result<Result<<Self::DstPtr as Ptr>::Persist, R>,
                Self::Error>;

    fn try_save<T: ?Sized + SavePtr<Self::SrcPtr, Self::DstPtr>>(
        &self,
        ptr: &<Self::SrcPtr as Ptr>::Persist,
        metadata: T::Metadata
    ) -> Result<Result<<Self::DstPtr as Ptr>::Persist, T::SavePtrPoll>,
                Self::Error>
    {
        self.try_save_raw(ptr, metadata, |valid_blob, zone| {
            T::deref_blob(valid_blob, zone)
              .init_save_ptr()
        })
    }

    /// Saves a value whose children have been saved.
    fn finish_save<T>(&mut self, value_poll: &T) -> Result<<Self::DstPtr as Ptr>::Persist, Self::Error>
        where T: EncodeBlob;
}

pub trait WriteBlob : Sized {
    type Ok;
    type Error;

    fn write_bytes(self, buf: &[u8]) -> Result<Self, Self::Error>;

    fn write_padding(mut self, len: usize) -> Result<Self, Self::Error> {
        for _ in 0 .. len {
            self = self.write_bytes(&[0])?;
        }
        Ok(self)
    }

    //fn write<Y: Zone, T: SavePoll<Y>>(self, value: &T) -> Result<Self, Self::Error>;
    fn finish(self) -> Result<Self::Ok, Self::Error>;
}

impl WriteBlob for Vec<u8> {
    type Error = !;
    type Ok = Self;

    fn write_bytes(mut self, buf: &[u8]) -> Result<Self, Self::Error> {
        self.extend_from_slice(buf);
        Ok(self)
    }

    fn write_padding(mut self, len: usize) -> Result<Self, Self::Error> {
        self.resize(self.len() + len, 0);
        Ok(self)
    }

    fn finish(self) -> Result<Self, !> {
        Ok(self)
    }
}

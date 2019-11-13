use super::*;

use core::marker::PhantomData;
use core::mem::ManuallyDrop;
use core::fmt;

use crate::marshal::Persist;

/// An owned pointer to a value in a `Zone`.
#[repr(C)]
pub struct Own<T: ?Sized + Pointee, Z: Zone> {
    marker: PhantomData<T>,
    ptr: ManuallyDrop<Z::Ptr>,
    metadata: T::Metadata,
}

unsafe impl<T: ?Sized + Pointee, Z: Zone> Persist for Own<T,Z>
where Z: Persist {}

impl<T: ?Sized + Pointee, Z: Zone> Own<T,Z> {
    pub unsafe fn from_raw_parts(ptr: Z::Ptr, metadata: T::Metadata) -> Self {
        Self {
            marker: PhantomData,
            ptr: ManuallyDrop::new(ptr),
            metadata,
        }
    }

    pub fn into_raw_parts(self) -> (Z::Ptr, T::Metadata) {
        let mut this = ManuallyDrop::new(self);
        let ptr = unsafe { (&mut *this.ptr as *mut Z::Ptr).read() };
        (ptr, this.metadata)
    }

    pub fn ptr(&self) -> &Z::Ptr {
        &self.ptr
    }

    pub fn metadata(&self) -> T::Metadata {
        self.metadata
    }
}

impl<T: ?Sized + Pointee, Z: Zone> Drop for Own<T,Z> {
    fn drop(&mut self) {
        unsafe {
            let ptr = (&mut *self.ptr as *mut Z::Ptr).read();
            Z::dealloc_own::<T>(ptr, self.metadata)
        }
    }
}

impl<T: ?Sized + Pointee, Z: Zone> fmt::Debug for Own<T,Z>
where T: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Z::fmt_debug_own(self, f)
    }
}

impl<T: ?Sized + Pointee, Z: Zone> fmt::Pointer for Own<T,Z>
where Z::Ptr: fmt::Pointer,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Pointer::fmt(&*self.ptr, f)
    }
}

/*
impl<T: ?Sized + Pointee, Z: Zone> Own<T,Z> {
    pub unsafe fn from_raw_parts(ptr: Z::Ptr, metadata: T::Metadata) -> Self {
        Self {
            marker: PhantomData,
            ptr, metadata
        }
    }

    pub fn metadata(&self) -> T::Metadata {
        self.metadata
    }

    pub fn ptr(&self) -> &Z::Ptr {
        &self.ptr
    }
}

pub enum OwnEncoder<T: ?Sized + Save<Z>, Z: Zone> {
    Own(Own<T,Z>),
    Save(<T as Save<Z>>::Save),
    Done {
        persist_ptr: Z::PersistPtr,
        metadata: T::Metadata,
    },
    Poisoned,
}

impl<T: ?Sized + Pointee, Z: Zone> Encode<Z> for Own<T,Z>
where T: Save<Z>
{
    const BLOB_LAYOUT: BlobLayout = BlobLayout::new(0);

    type Encode = OwnEncoder<T, Z>;

    fn encode(self) -> Self::Encode {
        OwnEncoder::Own(self)
    }
}

impl<T: ?Sized + Pointee, Z: Zone> EncodePoll for OwnEncoder<T,Z>
where T: Save<Z>
{
    type Zone = Z;
    type Target = Own<T, Z>;

    fn poll<S>(&mut self, ptr_saver: &mut S) -> Poll<Result<(), S::Error>>
        where S: Saver<Zone = Z>
    {
        match self {
            OwnEncoder::Own(_) => {
                if let OwnEncoder::Own(own) = mem::replace(self, OwnEncoder::Poisoned) {
                    todo!()
                } else {
                    unreachable!()
                }
            },
            OwnEncoder::Save(saver) => {
                match saver.poll(ptr_saver)? {
                    Poll::Ready((persist_ptr, metadata)) => {
                        mem::replace(self, OwnEncoder::Done { persist_ptr, metadata });
                        Ok(()).into()
                    },
                    Poll::Pending => Poll::Pending,
                }
            },
            OwnEncoder::Done { .. } => Ok(()).into(),
            OwnEncoder::Poisoned => panic!("{} poisoned", type_name::<Self>()),
        }
    }

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
        todo!()
    }
}
*/

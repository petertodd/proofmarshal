use std::mem;

pub mod blob;
use self::blob::*;

pub trait Load {
    type Error : std::error::Error;

    fn load<'a>(blob: Blob<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error>;
}

/*
pub trait Primitive : decode::Decode<!> + for<'a> encode::Encode<'a, !, Encoded=Self> {
    fn encode_primitive_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Ok, W::Error> {
        let state = self.make_encode_state();
        // FIXME: should poll with a dummy dumper here
        self.encode_blob(&state, dst)
    }
}

pub trait PtrValidator<Z> {
    type Error;

    fn validate_ptr<'a, T>(&self, ptr: &'a Z::PersistPtr, metadata: T::Metadata) -> Result<Option<&'a T::Persist>, Self::Error>
    where T: ?Sized + ValidatePointeeChildren<'a, Z>,
          Z: Zone;
}

pub trait Dumper<Y> : Sized {
    type Error;

    type WriteBlob : WriteBlob<Ok=Self::WriteBlobOk, Error=Self::WriteBlobError>;
    type WriteBlobOk;
    type WriteBlobError;

    type BlobPtr : 'static;

    /// Checks if the value behind a valid pointer has already been saved.
    ///
    /// On success, returns a persistent pointer. Otherwise, returns the dereferenced value so that
    /// the callee can save it.
    fn try_save_ptr<'a, T: ?Sized + Pointee>(&self, ptr: &'a ValidPtr<T, Y>) -> Result<Y::PersistPtr, &'a T>
	where Y: Zone;

    /// Saves a blob.
    fn save_blob(self,
        size: usize,
        f: impl FnOnce(Self::WriteBlob) -> Result<Self::WriteBlobOk, Self::WriteBlobError>
    ) -> Result<(Self, Self::BlobPtr), Self::Error>;

    fn blob_ptr_to_zone_ptr(ptr: Self::BlobPtr) -> Y::PersistPtr
        where Y: Zone;

    fn encode_value<'a, T>(self, value: &T, state: &T::State) -> Result<(Self, Self::BlobPtr), Self::Error>
        where T: encode::Encode<'a, Y>
    {
        self.save_blob(mem::size_of::<T::Encoded>(), |dst| {
            value.encode_blob(state, dst)
        })
    }
}

*/
#[cfg(test)]
mod tests {
    #[test]
    fn test() {
    }
}

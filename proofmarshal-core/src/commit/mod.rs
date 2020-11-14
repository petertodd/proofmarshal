//! Cryptographic commitments.

use std::any;
use std::convert::TryFrom;
use std::fmt;
use std::hash;
use std::marker::PhantomData;
use std::mem::{self, MaybeUninit};
use std::slice;

use hoard::prelude::*;
use hoard::owned::IntoOwned;
use hoard::blob::{Blob, BlobDyn, Bytes, BytesUninit};

mod impls;

pub mod sha256;
pub use self::sha256::*;

pub trait Digest : Primitive + Default + AsRef<[u8]> + AsMut<[u8]> + Eq
{
    type Hasher : Default + Hasher<Output = Self>;
}

pub trait Hasher {
    type Output;

    fn hash_bytes(&mut self, buf: &[u8]);

    fn hash_zeros(&mut self, len: usize) {
        for _ in 0 .. len {
            self.hash_bytes(&[0]);
        }
    }

    fn hash_blob<T: Blob>(&mut self, blob: &T) {
        if T::SIZE <= mem::size_of::<MaybeUninit<[T; 3]>>() {
            let mut buf: MaybeUninit<[T; 3]> = MaybeUninit::uninit();
            let buf = unsafe { slice::from_raw_parts_mut(
                &mut buf as *mut _ as *mut MaybeUninit<u8>,
                mem::size_of::<MaybeUninit<[T; 3]>>()
            ) };

            let dst = BytesUninit::<T>::try_from(&mut buf[0 .. T::SIZE]).unwrap();
            let dst = blob.encode_bytes(dst);

            self.hash_bytes(&dst);
        } else {
            // Panic, because this really doesn't make sense even with nonzero optimizations.
            unreachable!("blob encoding larger than 3 * mem::size_of::<{}>())", any::type_name::<T>())
        }
    }

    fn hash_commitment<T: Commit>(&mut self, value: &T) {
        self.hash_blob(&value.to_commitment())
    }

    fn finish(self) -> Self::Output;
}

/// A type for which there exists a canonical fixed-size commitment.
pub trait Commit {
    type Commitment : 'static + Blob;

    fn to_commitment(&self) -> Self::Commitment;

    fn encode_commitment_bytes<'a>(&self, dst: BytesUninit<'a, Self::Commitment>) -> Bytes<'a, Self::Commitment> {
        Blob::encode_bytes(&self.to_commitment(), dst)
    }

    fn hash_commitment_with<H: Hasher>(&self, mut hasher: H) -> H::Output {
        hasher.hash_blob(&self.to_commitment());
        hasher.finish()
    }
}

/// Variable-length commitments.
pub trait CommitRef {
    const HASH_COMMITMENT_METADATA: bool;
    type CommitmentDyn : 'static + ?Sized + BlobDyn;

    fn commitment_metadata(&self) -> <Self::CommitmentDyn as Pointee>::Metadata;

    fn encode_commitment_bytes_dyn<'a>(&self, dst: BytesUninit<'a, Self::CommitmentDyn>) -> Bytes<'a, Self::CommitmentDyn>;

    fn hash_commitment_dyn_with<H: Hasher>(&self, hasher: H) -> H::Output;
}

impl<T: ?Sized + Commit> CommitRef for T {
    const HASH_COMMITMENT_METADATA: bool = false;
    type CommitmentDyn = T::Commitment;

    fn commitment_metadata(&self) -> <Self::CommitmentDyn as Pointee>::Metadata {
    }

    fn encode_commitment_bytes_dyn<'a>(&self, dst: BytesUninit<'a, Self::CommitmentDyn>) -> Bytes<'a, Self::CommitmentDyn> {
        self.encode_commitment_bytes(dst)
    }

    fn hash_commitment_dyn_with<H: Hasher>(&self, hasher: H) -> H::Output {
        self.hash_commitment_with(hasher)
    }
}

#[macro_export]
macro_rules! impl_commit {
    ( $( $t:ty ),+ $(,)? ) => {$(
        impl $crate::commit::Commit for $t {
            type Commitment = $t;

            #[inline]
            fn to_commitment(&self) -> Self::Commitment {
                *self
            }
        }
    )+}
}

impl_commit! {
    !, (),
    bool,
    u8, u16, u32, u64, u128,
    i8, i16, i32, i64, i128,
}

/// A commitment that has been hashed to reduce its length (if necessary).
///
/// A `HashCommit<T, D>` is a wrapper around the digest value, `D`. If the `T` blob bytes are
/// greater than the length of `D`, the bytes will be hashed:
///
/// ```text
/// // FIXME
/// ```
///
/// ...otherwise the blob bytes will be simply used as the digest value, verbatim:
///
/// ```text
/// // FIXME
/// ```
#[repr(transparent)]
pub struct HashCommit<T: ?Sized, D: Digest = Sha256Digest> {
    marker: PhantomData<T>,
    digest: D,
}

impl<T: ?Sized, D: Digest> Clone for HashCommit<T, D> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: ?Sized, D: Digest> Copy for HashCommit<T, D> {}

impl<U: ?Sized, T: ?Sized, D: Digest> AsRef<U> for HashCommit<T, D>
where D: AsRef<U>
{
    fn as_ref(&self) -> &U {
        self.digest.as_ref()
    }
}

impl<U: ?Sized, T: ?Sized, D: Digest> AsMut<U> for HashCommit<T, D>
where D: AsMut<U>
{
    fn as_mut(&mut self) -> &mut U {
        self.digest.as_mut()
    }
}

impl<T: ?Sized + BlobDyn, D: Digest> HashCommit<T, D> {
    pub fn new<U>(value: &U) -> Self
        where U: ?Sized + CommitRef<CommitmentDyn=T>
    {
        let metadata = value.commitment_metadata();
        let size = U::CommitmentDyn::try_size(metadata)
                                    .expect("valid metadata");

        if size <= mem::size_of::<D>() {
            let mut digest = D::default();
            let dst = BytesUninit::<T>::from_bytes(
                &mut digest.as_mut()[.. size],
                metadata,
            ).unwrap();

            value.encode_commitment_bytes_dyn(dst);

            Self::from_digest(digest)
        } else {
            let digest = value.hash_commitment_dyn_with(D::Hasher::default());
            Self::from_digest(digest)
        }
    }
}

impl<T: ?Sized, D: Digest> HashCommit<T, D> {
    pub fn from_digest(digest: D) -> Self {
        Self {
            marker: PhantomData,
            digest,
        }
    }

    pub fn digest(&self) -> D {
        self.digest
    }

    pub fn cast<U>(self) -> HashCommit<U, D> {
        HashCommit::from_digest(self.digest())
    }
}

impl<T: ?Sized, D: Digest + Default> Default for HashCommit<T, D> {
    fn default() -> Self {
        Self::from_digest(D::default())
    }
}

impl<T: ?Sized, D: Digest + PartialEq> PartialEq for HashCommit<T, D> {
    fn eq(&self, other: &Self) -> bool {
        self.digest == other.digest
    }
}
impl<T: ?Sized, D: Digest + Eq> Eq for HashCommit<T, D> {}

impl<T: ?Sized, D: Digest + fmt::Debug> fmt::Debug for HashCommit<T, D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.digest.fmt(f)
    }
}

impl<T: ?Sized, D: Digest + fmt::Display> fmt::Display for HashCommit<T, D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.digest.fmt(f)
    }
}

impl<T: ?Sized, D: Digest + hash::Hash> hash::Hash for HashCommit<T, D> {
    fn hash<H: hash::Hasher>(&self, hasher: &mut H) {
        self.digest.hash(hasher)
    }
}

impl<T: ?Sized + 'static, D: Digest> Primitive for HashCommit<T, D> {
    const BLOB_SIZE: usize = D::BLOB_SIZE;
    type DecodeBytesError = D::DecodeBytesError;

    fn encode_blob_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        dst.write_struct()
           .write_field(&self.digest)
           .done()
    }

    fn decode_blob_bytes(src: Bytes<'_, Self>) -> Result<Self, Self::DecodeBytesError> {
        let mut fields = src.struct_fields();
        let digest = fields.trust_field()?;
        fields.assert_done();
        Ok(Self::from_digest(digest))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[track_caller]
    fn t<T: Commit>(value: T, expected: &[u8]) {
        let d = HashCommit::<T::Commitment>::new(&value);
        assert_eq!(d.as_ref(), expected);
    }

    #[test]
    fn short_hash_commit() {
        t((), &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        t(1u8, &[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        t(true, &[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

        t([1u8,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32],
         &[1u8,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32])
    }

    #[test]
    fn long_hash_commit() {
        t([1u8,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33],
         &[49, 176, 60, 110, 174, 212, 117, 221, 227, 69, 177, 206, 130, 147, 185, 174, 139, 252, 123, 217, 102, 101, 151, 221, 242, 140, 24, 250, 115, 213, 196, 244])
    }
}

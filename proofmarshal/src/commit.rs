use core::hash;

use persist::Persist;
use nonzero::NonZero;

use crate::digest::{Digest, CryptDigest};

pub trait Commit {
    type Commitment : 'static + Copy + NonZero + Persist + Default + Eq + Ord + hash::Hash + Send + Sync;

    fn commit(&self) -> Self::Commitment;
}


// blanket impls

/// Digests commit to themselves
impl<T: 'static + ?Sized, D: CryptDigest> Commit for Digest<T,D> {
    type Commitment = Self;
    fn commit(&self) -> Self::Commitment {
        self.clone()
    }
}

impl<T: ?Sized + Commit> Commit for &'_ T {
    type Commitment = T::Commitment;

    fn commit(&self) -> Self::Commitment {
        (**self).commit()
    }
}

impl<T: ?Sized + Commit> Commit for &'_ mut T {
    type Commitment = T::Commitment;

    fn commit(&self) -> Self::Commitment {
        (**self).commit()
    }
}

impl<T: ?Sized + Commit> Commit for Box<T> {
    type Commitment = T::Commitment;

    fn commit(&self) -> Self::Commitment {
        (**self).commit()
    }
}


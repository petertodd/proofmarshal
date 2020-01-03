//! Fact validation.

use std::cell::UnsafeCell;
use std::fmt;
use std::mem::MaybeUninit;
use std::num::NonZeroU8;
use std::ops;
use std::ptr;
use std::sync::atomic::{AtomicU8, Ordering, spin_loop_hint};

use super::*;

/// A fact that has been verified to be true.
pub struct Valid<T: Fact<Z>, Z: Zone = !>(Maybe<T, Z>);

impl<Z: Zone, T: Fact<Z>> ops::Deref for Valid<T,Z> {
    type Target = T;

    fn deref(&self) -> &T {
        todo!()
    }
}

#[cfg(test)]
mod tests {
}

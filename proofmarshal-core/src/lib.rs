//! Marshalling of merkelized cryptographic proofs - what the cool kids call "blockchain".

#![feature(arbitrary_self_types)]
#![feature(never_type)]
#![feature(rustc_attrs)]
#![feature(slice_ptr_len)]
#![feature(unwrap_infallible)]

#![allow(incomplete_features)]
#![feature(const_generics)]
#![feature(try_trait)]

#![allow(unused_imports)]
#![allow(dead_code)]

pub mod commit;
pub mod hashbag;

pub mod collections;

// FIXME: this shouldn't be public
#[doc(hidden)]
#[macro_export]
macro_rules! unreachable_unchecked {
    ($($arg:tt)*) => {
        if cfg!(debug_assertions) {
            panic!($($arg)*)
        } else {
            ::core::hint::unreachable_unchecked()
        }
    }
}

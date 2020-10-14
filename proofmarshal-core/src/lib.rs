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

pub mod collections;

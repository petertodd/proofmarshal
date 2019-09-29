//! Marshalling of merkelized cryptographic proofs - what the cool kids call "blockchain".

#![feature(never_type)]
#![feature(maybe_uninit_extra)]
#![feature(manually_drop_take)]

pub mod arena;

pub mod digest;
pub mod fact;
pub mod commit;

pub mod prelude {
    pub use super::arena::{Own, Ptr, MutPtr, Alloc};
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

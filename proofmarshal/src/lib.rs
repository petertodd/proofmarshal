//! Marshalling of merkelized cryptographic proofs - what the cool kids call "blockchain".

#![feature(never_type)]
#![feature(maybe_uninit_extra)]
#![feature(manually_drop_take)]

pub mod verbatim;

pub mod ptr;

pub mod digest;
pub mod commit;

pub mod fact;

pub mod seal;

pub mod prelude {
    pub use super::{
        ptr::{
            Ptr,Own,Alloc,
            TryGet,Get,TryGetMut,GetMut,
            heap::Heap,
        },
        commit::Commit,
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

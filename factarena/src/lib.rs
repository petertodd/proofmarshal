//! Uniform storage of both volatile and persistent data.

#![feature(alloc_layout_extra)]
#![feature(never_type)]
#![feature(slice_from_raw_parts)]
#![feature(manually_drop_take)]
#![feature(maybe_uninit_extra)]

// self-contained utility stuff
pub mod util;

// These deal with in-memory data only.
pub mod pointee;

pub mod marshal;

//pub mod arena;
//pub mod pile;

//pub mod collections;

/*
pub mod prelude {
    pub use super::{
        arena::{
            Arena, Alloc,
            refs::Ref,
            own::Own,
            heap::Heap,
            marshal::Type,
            persist::leint::Le,
        },
    };
}
*/


#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

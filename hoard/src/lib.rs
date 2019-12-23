//! # Hoard
//!
//! Hoard is a framework for persistently storing arbitrary data-structures to disk with
//! copy-on-write semantics. Hoard achives this by generalizing the notion of a pointer: rather
//! than solely pointing to volatile memory, pointers can be, for example, be an offset within a
//! memory-mapped database file, or a hash digest obtained remotely. This is combined with highly
//! efficient and robust serialization and deserialization, based on simple fixed-size formats that
//! match in-memory representations sufficiently closely to allow data to be directly memory-mapped
//! from disk.
//!
//! This means that like the Serde framework, "hoarded" datatypes can be accessed in the same way
//! as any other Rust data: structs and enums are simply structs and enums. Unlike Serde, Hoard's
//! support for pointers means you can load data on demand: a non-volatile tree stored in a file
//! can be accessed in almost exactly the same way as a volatile tree stored on the heap.
//!
//! Mutation is via copy-on-write: mutating data behind mutable pointers transparently makes a
//! mutable copy on the heap. When you're ready to save the data, the changes are written to disk
//! in an atomic transaction; unmodified data is left unchanged.

#![feature(associated_type_bounds)]
#![feature(alloc_layout_extra)]
#![feature(slice_from_raw_parts)]
#![feature(manually_drop_take)]
#![feature(arbitrary_self_types)]
#![feature(const_if_match)]
#![feature(optin_builtin_traits)]
#![feature(never_type)]
#![feature(backtrace)]

#![allow(incomplete_features)]
#![feature(const_generics)]

#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]

#[cfg(not(target_pointer_width = "64"))]
compile_error!("64-bit pointers required");

#[cfg(not(target_endian = "little"))]
compile_error!("little endian required");

pub mod coerce;
pub mod pointee;

pub mod zone;
pub mod blob;
pub mod load;
pub mod save;
pub mod marshal;

pub mod heap;

pub mod pile;

pub mod linkedlist;

/// Prelude
pub mod prelude {
    pub use leint::Le;

    pub use crate::zone::{
        Alloc, Zone,
        TryGet, Get,
        OwnedPtr,

        refs::{Own, Ref, RefMut},
    };

    /*
    pub use crate::marshal::{
        en::Save, de::Load,
    };

    pub use crate::refs::{
        Own,
        Ref,
        RefMut,
    };
    */
}

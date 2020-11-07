#![feature(never_type)]

#![feature(maybe_uninit_uninit_array)]
#![feature(maybe_uninit_extra)]
#![feature(associated_type_bounds)]
#![feature(unwrap_infallible)]
#![feature(arbitrary_self_types)]

#![feature(rustc_attrs)]

#![allow(incomplete_features)]
#![feature(const_generics)]

#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]

pub mod owned;

pub mod maybevalid;

//pub mod scalar;
pub mod pointee;
pub mod blob;

pub mod ptr;

pub mod load;
pub mod save;

pub mod primitive;
pub mod bag;

/*
pub mod offset;
pub mod pile;
*/

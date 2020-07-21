//! Implementations on foreign types.

use super::*;

use crate::pointee::Pointee;
use crate::blob::*;
use crate::load::*;
use crate::save::*;
use crate::scalar::*;
use crate::ptr::*;

pub mod never;
pub mod scalars;
pub mod array;
//pub mod option;

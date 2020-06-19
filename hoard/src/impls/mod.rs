//! Implementations on foreign types.

use super::*;

use crate::pointee::Pointee;
use crate::blob::*;
use crate::scalar::Scalar;
use crate::load::*;
use crate::save::*;
use crate::zone::Zone;
use crate::refs::Ref;
use crate::writebytes::WriteBytes;

pub mod never;

pub mod scalars;
pub mod array;
pub mod option;

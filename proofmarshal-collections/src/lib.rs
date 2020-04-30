#![feature(never_type)]

#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]

use proofmarshal_core::*;

pub mod merklesum;
pub mod tree;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

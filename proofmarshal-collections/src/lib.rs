#![feature(never_type)]

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

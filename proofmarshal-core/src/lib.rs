//! Marshalling of merkelized cryptographic proofs - what the cool kids call "blockchain".

#![feature(never_type)]
#![feature(specialization)]

#![allow(incomplete_features)]
#![feature(const_generics)]
#![feature(const_if_match)]

#![allow(unused_imports)]
#![allow(dead_code)]

pub mod commit;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

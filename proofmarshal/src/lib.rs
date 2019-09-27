//! Marshalling of merkelized cryptographic proofs - what the cool kids call "blockchain".

#![feature(never_type)]

pub mod digest;
pub mod commit;

pub mod bitcoin;

pub mod maybe;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

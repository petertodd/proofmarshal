//! Marshalling of merkelized cryptographic proofs - what the cool kids call "blockchain".

#![feature(never_type)]

pub mod fact;
pub mod commit;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

#![feature(never_type)]

use leint::Le;
use hoard_derive::Primitive;

#[derive(Primitive)]
#[repr(C)]
pub struct Outpoint {
    txid: [u8;32],
    n: Le<u32>,
}

#[derive(Primitive)]
#[repr(C)]
pub struct Foo(u8,bool);

#[cfg(tests)]
mod tests {
    #[test]
    fn test() {
    }
}

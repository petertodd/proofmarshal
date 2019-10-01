use verbatim_derive::Verbatim;
use proofmarshal_derive::Commit;

use proofmarshal::prelude::*;
use proofmarshal::digest::Digest;
use proofmarshal::commit::Commit;

#[derive(Verbatim, Commit)]
pub struct Foo {
    bar: u8,
    car: bool,
}

#[derive(Verbatim, Commit)]
pub struct Bar<T: 'static> {
    generic: T,
    foo: Foo,
}

#[derive(Verbatim, Commit)]
pub struct BarTuple<T: 'static>(T, Foo, Bar<T>);

#[derive(Verbatim, Commit)]
pub struct Unit;


#[derive(Verbatim, Commit, Debug)]
pub enum Enum {
    A(u8),
    B(u16),
    C(u32),
    D(bool,u16,u32),
    E(bool,u16,u32),
    F {
        a: bool,
        b: u16,
        c: u32,
    },
}

pub fn test_foo_commit(x: &Foo) -> Digest<Foo> {
    x.commit()
}

pub fn test_unit_commit(x: &Unit) -> Digest<Unit> {
    x.commit()
}

pub fn test_enum_commit(x: &Enum) -> Digest<Enum> {
    x.commit()
}

pub fn test_bar_commit(x: &Bar<[u8;32]>) -> Digest<Bar<[u8;32]>> {
    x.commit()
}

pub fn test_bar_commit2(x: &Bar<[u16;32]>) -> Digest<Bar<[u16;32]>> {
    x.commit()
}

#[cfg(test)]
mod tests {
    use super::*;

    use verbatim::Verbatim;

    #[test]
    fn test() {
        let buf = [5,0,0,0,0,0,0,0];

        let _ = dbg!(<Enum as Verbatim>::decode(&buf, &mut ()));
    }
}

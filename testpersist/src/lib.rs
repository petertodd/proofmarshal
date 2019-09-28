use persist_derive::*;

#[repr(C)]
#[derive(Persist)]
pub struct Foo {
    bar: u8,
    car: bool,
}

#[repr(C)]
#[derive(Persist)]
pub struct Bar<T> {
    generic: T,
    foo: Foo,
}

#[repr(C)]
#[derive(Persist)]
pub struct BarTuple<T>(T, Foo, Bar<T>);

#[repr(C)]
#[derive(Persist)]
pub struct Unit;

#[cfg(test)]
mod tests {
    use super::*;

    use persist::Persist;

    #[test]
    fn test() {
        let f = Foo { bar: 12, car: true };

        dbg!(f.canonical_bytes());
    }
}

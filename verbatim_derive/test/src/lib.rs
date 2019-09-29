use verbatim_derive::Verbatim;

#[derive(Verbatim)]
pub struct Foo {
    bar: u8,
    car: bool,
}

#[derive(Verbatim)]
pub struct Bar<T> {
    generic: T,
    foo: Foo,
}

#[derive(Verbatim)]
pub struct BarTuple<T>(T, Foo, Bar<T>);

#[derive(Verbatim)]
pub struct Unit;


#[derive(Verbatim, Debug)]
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

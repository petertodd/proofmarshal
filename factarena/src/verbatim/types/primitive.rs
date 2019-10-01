use super::*;

pub trait Value {
    fn kind(&self) -> Kind;
}

impl Value for ! {
    fn kind(&self) -> Kind { match *self {} }
}

pub enum Kind {
    Unit,
    Bool(bool),
    U8(u8),
}


impl Value for bool {
    fn kind(&self) -> Kind {
        Kind::Bool(*self)
    }
}

impl Value for u8 {
    fn kind(&self) -> Kind {
        Kind::U8(*self)
    }
}

impl<P> super::Value<P> for u8 {
    type Primitive = Self;
    type TupleItems = !;

    fn kind(&self) -> super::Kind<&Self::Primitive, &!, &P> {
        super::Kind::Primitive(self)
    }
}

impl super::OwnedValue for u8 {
    fn into_kind(self) -> super::Kind<Self, !, !> {
        super::Kind::Primitive(self)
    }
}

/*
impl super::ValueRef<'_> for u8 {
    type Primitive = Self;
    type TupleItems = !;

    fn get(&'_ self) -> super::Kind<Self::Primitive, !> {
        super::Kind::Primitive(*self)
    }
}
*/

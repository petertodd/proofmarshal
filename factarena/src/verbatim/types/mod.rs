//! Standardized types

pub mod primitive;
pub mod tuple;

use self::tuple::*;

#[derive(Debug)]
pub enum Kind<T, I, P=!> {
    Primitive(T),
    Tuple(I),
    Ptr(P),
}

pub trait Value<P=!> {
    type Primitive : primitive::Value;
    type TupleItems : tuple::Items<P>;

    fn kind(&self) -> Kind<&Self::Primitive, &Self::TupleItems, &P>;
}

pub trait OwnedValue<P=!> : Value<P> {
    fn into_kind(self) -> Kind<Self::Primitive, Self::TupleItems, P>;
}

impl<P> Value<P> for ! {
    type Primitive = !;
    type TupleItems = !;

    fn kind(&self) -> Kind<&Self::Primitive, &Self::TupleItems, &P> { match *self {} }
}

impl<P> OwnedValue<P> for ! {
    fn into_kind(self) -> Kind<Self::Primitive, Self::TupleItems, P> { match self {} }
}



impl<'a,P,T> Value<P> for &'a T
where T: Value<P>
{
    type Primitive = T::Primitive;
    type TupleItems = T::TupleItems;

    fn kind(&self) -> Kind<&Self::Primitive, &Self::TupleItems, &P> {
        (**self).kind()
    }
}


pub trait Encode<P> {
    type Ok;
    type Error;

    fn encode<'a, T: Value<P>>(self, value: &T) -> Result<Self::Ok, Self::Error>;
}

pub trait AsValue<'a, P> {
    type Value : Value<P>;

    fn as_value(&'a self) -> Self::Value;
}

impl<'a,P,T> AsValue<'a, P> for &'a T
where T: AsValue<'a, P>
{
    type Value = T::Value;
    fn as_value(&'a self) -> Self::Value {
        (**self).as_value()
    }
}

pub trait Verbatim<P> {
    type Value : OwnedValue<P>;

    fn into_value(self) -> Self::Value;
    fn from_value(value: Self::Value) -> Self;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Foo(u8,u8);

    impl<'a, P> AsValue<'a, P> for Foo {
        type Value = Tuple<Item<&'a u8, Item<&'a u8, ()>>>;

        fn as_value(&'a self) -> Self::Value {
            Tuple(Item(&self.0, Item(&self.1, ())))
        }
    }

    struct Bar<T>(T,Foo);

    impl<'a, P, T> AsValue<'a, P> for Bar<T>
    where T: AsValue<'a, P>
    {
        type Value = Tuple<Item<T::Value,
                           Item<<Foo as AsValue<'a,P>>::Value,
                                 ()>>>;

        fn as_value(&'a self) -> Self::Value {
            Tuple(Item(self.0.as_value(),
                  Item(AsValue::<P>::as_value(&self.1),
                  ())))
        }
    }

    #[test]
    fn test() {
        let bar = Bar(Foo(1,2), Foo(3,4));

        dbg!(AsValue::<!>::as_value(&bar));
    }
}

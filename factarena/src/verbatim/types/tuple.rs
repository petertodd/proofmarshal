use super::*;

#[derive(Default, Debug, Clone, Copy)]
pub struct Tuple<I=()>(pub I);

#[derive(Debug, Clone, Copy)]
pub struct Item<T, N=()>(pub T,pub N);


impl<P, I: Items<P>> Value<P> for Tuple<I> {
    type Primitive = !;
    type TupleItems = I;

    fn kind(&self) -> Kind<&!, &I, &P> {
        Kind::Tuple(&self.0)
    }
}

impl<P, I: OwnedItems<P>> OwnedValue<P> for Tuple<I> {
    fn into_kind(self) -> Kind<!, I, P> {
        Kind::Tuple(self.0)
    }
}


pub trait Items<P=!> {
    type Item : Value<P>;
    type Next : Items<P>;

    fn get(&self) -> Option<(&Self::Item, &Self::Next)>;
}

impl<P> Items<P> for ! {
    type Item = !;
    type Next = !;

    fn get(&self) -> Option<(&Self::Item, &Self::Next)> { match *self {} }
}

impl<P> Items<P> for () {
    type Item = !;
    type Next = !;

    fn get(&self) -> Option<(&Self::Item, &Self::Next)> {
        None
    }
}

impl<P,T,N> Items<P> for Item<T,N>
where T: Value<P>,
      N: Items<P>,
{
    type Item = T;
    type Next = N;

    fn get(&self) -> Option<(&Self::Item, &Self::Next)> {
        Some((&self.0, &self.1))
    }
}

pub trait OwnedItems<P=!> : Items<P> {
    fn take(self) -> Option<(Self::Item, Self::Next)>;
}

impl<P> OwnedItems<P> for ! {
    fn take(self) -> Option<(Self::Item, Self::Next)> { match self {} }
}

impl<P> OwnedItems<P> for () {
    fn take(self) -> Option<(Self::Item, Self::Next)> {
        None
    }
}

impl<P,T,N> OwnedItems<P> for Item<T,N>
where T: Value<P>,
      N: Items<P>,
{
    fn take(self) -> Option<(Self::Item, Self::Next)> {
        Some((self.0, self.1))
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let t = Tuple(Item(10u8,Item(11u8,())));

        dbg!(Value::<!>::kind(&t));
        dbg!(OwnedValue::<!>::into_kind(t));

        let a = 10u8;
        let b = 11u8;

        let _t_ref = Tuple(Item(&a, Item(&b,())));
    }
}

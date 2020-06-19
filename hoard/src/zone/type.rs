//! Saving values.
use crate::pointee::Pointee;

use crate::zone::{Zone, Ptr, Own};
use crate::bag::Bag;

/// The type of a type when saved in a specific zone.
pub trait Type<Zone> : Pointee {
    type Type : ?Sized + Pointee<Metadata = <Self as Pointee>::Metadata>;
}

macro_rules! impl_saved_for_primitives {
    ($( $t:ty, )* ) => {$(
        impl<Z> Type<Z> for $t {
            type Type = $t;
        }
    )*}
}

impl_saved_for_primitives! {
    !, (), bool,
    u8, u16, u32, u64, u128,
    i8, i16, i32, i64, i128,
}

impl<Z, T: Type<Z>> Type<Z> for Option<T>
where T::Type: Sized,
{
    type Type = Option<T::Type>;
}

impl<Z, T: Type<Z>, const N: usize> Type<Z> for [T; N]
where T::Type: Sized
{
    type Type = [T::Type; N];
}

impl<Z, T: Type<Z>> Type<Z> for [T]
where T::Type: Sized
{
    type Type = [T::Type];
}

impl<Z: Zone, T: ?Sized + Pointee, P: Ptr> Type<Z> for Own<T, P>
where T: Type<Z>
{
    type Type = Own<T::Type, Z::Ptr>;
}

impl<Y: Zone, T: ?Sized + Pointee, Z, P: Ptr, M: 'static> Type<Y> for Bag<T, Z, P, M>
where T: Type<Y>,
{
    type Type = Bag<T::Type, Y, Y::Ptr, M>;
}

/*
/// Saves a *copy* of a value in a new zone.
pub trait Save<Q, R> : Saved<R> + Pointee {
    type SavePoll : SavePoll<Q, R, Target = Self>;

    //fn init_save(&self, dst: &mut D) -> Result<Self::SavePoll, ;
}

pub trait SavePoll<Q, R> {
    type Target : ?Sized + Pointee;
}

impl<R, T: Saved<R>> Saved<R> for Option<T>
where T::Saved: Sized
{
    type Saved = Option<T::Saved>;
}

impl<Q, R, T: Save<Q, R>> Save<Q, R> for Option<T>
where T::Saved: Sized,
{
    type SavePoll = Option<T::SavePoll>;
}

impl<Q, R, T: SavePoll<Q, R>> SavePoll<Q, R> for Option<T>
where T::Target: Sized,
{
    type Target = Option<T::Target>;
}

pub trait SaveBlob<Q, R> : Save<Q, R> {
    fn save_blob(poll: &Self::SavePoll) -> Vec<u8>;
}
*/

/*



pub struct Foo<P: Ptr> {
    inner: Own<u8, P, ()>,
}

impl<R: Ptr, P: Ptr> Saved<R> for Foo<P> {
    type Saved = Foo<R>;
}
*/

/*
pub trait Save<Q, R> {
    type SavePoll : SavePoll<Q, R> + SaveBlob;
    fn init_save(&self, dst: &impl SavePtr<Source=Q, Target=R>) -> Self::SavePoll;
}

pub trait SavePoll<Q, R> {
    fn save_poll<D>(&mut self, dst: D) -> Result<D, D::Error>
        where D: SavePtr<Source=Q, Target=R>;
}


pub trait Saver : Sized {
    type Source;
    type Target;
    type Error;

    /// Tries to directly coerce a `Source` pointer into a `Target` pointer.
    unsafe fn try_coerce<'a, T: ?Sized>(&self, ptr: &'a Self::Source, metadata: T::Metadata) -> Result<Self::Target, &'a T>
        where T: Pointee;

    fn try_save_ptr(self, saver: &impl SaveBlob) -> Result<(Self, Self::Target), Self::Error>;
}
*/

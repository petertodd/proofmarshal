//! Saving values.
use crate::pointee::Pointee;

use crate::zone::{Ptr, Own};
use crate::bag::Bag;

pub trait Saved<R> : Pointee {
    type Saved : ?Sized + Pointee<Metadata = <Self as Pointee>::Metadata>;
}

macro_rules! impl_saved_for_primitives {
    ($( $t:ty, )* ) => {$(
        impl<R> Saved<R> for $t {
            type Saved = $t;
        }
    )*}
}

impl_saved_for_primitives! {
    !, (), bool,
    u8, u16, u32, u64, u128,
    i8, i16, i32, i64, i128,
}

impl<R, T: Saved<R>> Saved<R> for Option<T>
where T::Saved: Sized,
{
    type Saved = Option<T::Saved>;
}

impl<R, T: Saved<R>, const N: usize> Saved<R> for [T; N]
where T::Saved: Sized
{
    type Saved = [T::Saved; N];
}

impl<R, T: Saved<R>> Saved<R> for [T]
where T::Saved: Sized
{
    type Saved = [T::Saved];
}

impl<R: Ptr, T: ?Sized + Pointee, P: Ptr> Saved<R> for Own<T, P>
where T: Saved<R>
{
    type Saved = Own<T::Saved, R>;
}

impl<R: Ptr, T: ?Sized + Pointee, P: Ptr, Z, M: 'static> Saved<R> for Bag<T, P, Z, M>
where T: Saved<R>,
      Z: Saved<R, Saved: Sized>,
{
    type Saved = Bag<T::Saved, R, Z::Saved, M>;
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

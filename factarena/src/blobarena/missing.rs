use core::marker::PhantomData;

use super::*;

/// Arena for Missing values
#[derive(Default,Debug)]
pub struct Missing<P> {
    marker: PhantomData<P>,
}

#[derive(Default,Debug)]
pub struct Error;

impl<P: Dealloc> Arena for Missing<P> {
    type Ptr = P;
    type Error = Error;

    fn try_load_ptr<'p, T: ?Sized + Load<Self>>(&self, _ptr: &'p Ptr<T,Self::Ptr>) -> Result<Ref<'p, T>, T::Error> {
        unimplemented!()
    }
}

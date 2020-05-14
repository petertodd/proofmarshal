use std::fmt;
use std::marker::PhantomData;
use std::mem::{self, ManuallyDrop};
use std::ops;

use thiserror::Error;

use crate::load::*;
use crate::save::*;
use crate::primitive::*;

use super::*;

pub struct Bag<T: ?Sized + Pointee, P: Ptr> {
    marker: PhantomData<Box<T>>,
    inner: Fat<T, P>,
}

impl<T: ?Sized + Pointee, P: Ptr> Bag<T,P> {
    pub unsafe fn new_unchecked(ptr: Fat<T,P>) -> Self {
        Self {
            marker: PhantomData,
            inner: ptr,
        }
    }

    pub fn into_inner(self) -> Fat<T,P> {
        let this = ManuallyDrop::new(self);

        unsafe { std::ptr::read(&this.inner) }
    }

    pub unsafe fn raw_mut(&mut self) -> &mut P {
        &mut self.inner.raw
    }
}

impl<T: ?Sized + Pointee, P: Ptr> Drop for Bag<T, P> {
    fn drop(&mut self) {
        unsafe {
            self.inner.raw.dealloc::<T>(self.inner.metadata)
        }
    }
}

impl<T: ?Sized + Pointee, P: Ptr> ops::Deref for Bag<T, P> {
    type Target = Fat<T,P>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}


impl<T: ?Sized + Pointee, P: Ptr> fmt::Debug for Bag<T, P>
where T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match unsafe { P::try_get_dirty_unchecked::<T>(&self.raw, self.metadata) } {
            Ok(value) => value.fmt(f),
            Err(persist_ptr) => persist_ptr.fmt(f),
        }
    }
}

#[derive(Error, Debug)]
pub enum LoadBagError<P: std::fmt::Debug, M: std::fmt::Debug, L: std::fmt::Debug> {
    #[error("invalid pointer: {0:?}")]
    Pointer(P),

    #[error("invalid metadata: {0:?}")]
    Metadata(M),

    #[error("layout error: {0:?}")]
    Layout(L),
}

#[derive(Debug)]
pub enum LoadBagState<P, M, S> {
    Ready {
        ptr: P,
        metadata: M,
    },
    Poll(S),
    Done,
}

impl<T: ?Sized + Pointee, P: Ptr> Load for Bag<T, P>
where T: Load, P: Load,
{
    type Error = LoadBagError<P::Error, <T::Metadata as Load>::Error, T::LayoutError>;

    fn load<'a>(blob: BlobCursor<'a, Self>) -> Result<ValidBlob<'a, Self>, Self::Error> {
        todo!()
    }
}

/*
    fn init_validate_state(&self) -> Self::State {
        match unsafe { P::try_get_dirty_unchecked::<T>(&self.inner.raw, self.inner.metadata) } {
            Ok(_) => LoadBagState::Done,
            Err(ptr) => LoadBagState::Ready {
                ptr: todo!(),
                metadata: self.inner.metadata,
            }
        }
    }

    fn poll<V: ValidatePtr<P>>(state: &mut Self::State, validator: &V) -> Result<(), V::Error>
    {
        loop {
            *state = match state {
                LoadBagState::Ready { ptr, metadata } => {
                    if let Some(value) = validator.validate_ptr::<T>(ptr.as_ref(), *metadata)? {
                        LoadBagState::Poll(value.init_validate_state())
                    } else {
                        LoadBagState::Done
                    }
                },
                LoadBagState::Poll(state) => {
                    //T::poll(state, validator)?;
                    LoadBagState::Done
                },
                LoadBagState::Done => break Ok(()),
            };
        }
    }
}
*/

impl<R: Ptr, T: ?Sized + Pointee, P: Ptr> Saved<R> for Bag<T, P>
where T: Saved<R>
{
    type Saved = Bag<T::Saved, R>;
}

#[derive(Debug)]
pub enum SaveBagState<'a, R, T: ?Sized, S> {
    Ready,
    Poll {
        value: &'a T,
        value_state: S,
    },
    SaveBlob {
        value: &'a T,
        value_state: S,
    },
    Done(R),
}

impl<'a, Q: 'a, R, T: ?Sized + Pointee, P: Ptr> Save<'a, Q, R> for Bag<T, P>
where T: 'a + Save<'a, Q, R>,
      P: AsPtr<Q>,
      R: Ptr + Primitive,
      R::Saved: Sized,
{
    type State = SaveBagState<'a, R, T, T::State>;

    fn init_save_state(&'a self) -> Self::State {
        SaveBagState::Ready
    }

    fn save_poll<D: SavePtr<Q, R>>(&'a self, state: &mut Self::State, mut dst: D) -> Result<D, D::Error> {
        loop {
            *state = match state {
                SaveBagState::Ready => {
                    match unsafe { dst.try_save_ptr::<T>(self.raw.as_ptr(), self.metadata) } {
                        Ok(q_ptr) => SaveBagState::Done(q_ptr),
                        Err(value) => SaveBagState::Poll {
                            value_state: value.init_save_state(),
                            value,
                        }
                    }
                },
                SaveBagState::Poll { value, value_state } => {
                    dst = value.save_poll(value_state, dst)?;

                    let value_state = mem::replace(value_state, value.init_save_state());

                    SaveBagState::SaveBlob { value, value_state }
                },
                SaveBagState::SaveBlob { value, value_state } => {
                    let (d, r_ptr) = dst.save::<T>(value, value_state)?;
                    dst = d;
                    SaveBagState::Done(r_ptr)
                },
                SaveBagState::Done(_) => break Ok(dst),
            }
        }
    }

    fn save_blob<W: SaveBlob>(&'a self, state: &Self::State, dst: W) -> Result<W::Done, W::Error> {
        let dst = dst.alloc(mem::size_of::<Self::Saved>())?;
        self.encode_blob(state, dst)
    }

    fn encode_blob<W: WriteBlob>(&'a self, state: &Self::State, dst: W) -> Result<W::Done, W::Error> {
        if let SaveBagState::Done(r_ptr) = state {
            dst.write_primitive(r_ptr)?
               .write_primitive(&self.metadata)?
               .done()
        } else {
            panic!()
        }
    }
}

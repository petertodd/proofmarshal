//! Contextual validation.

use std::any::TypeId;
use std::marker::PhantomData;
use std::mem;
use std::error::Error;
use std::ops;

mod impls;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Poll<T, P> {
    Ready(T),
    Pending(P),
}

impl<T, E, P> ops::Try for Poll<Result<T, E>, P> {
    type Ok = Poll<T, P>;
    type Error = E;

    fn into_result(self) -> Result<Self::Ok, Self::Error> {
        match self {
            Poll::Ready(Ok(t)) => Ok(Poll::Ready(t)),
            Poll::Ready(Err(e)) => Err(e),
            Poll::Pending(p) => Ok(Poll::Pending(p)),
        }
    }

    fn from_error(err: E) -> Self {
        Poll::Ready(Err(err))
    }

    fn from_ok(ok: Poll<T, P>) -> Self {
        match ok {
            Poll::Ready(v) => Poll::Ready(Ok(v)),
            Poll::Pending(p) => Poll::Pending(p),
        }
    }
}

pub unsafe trait Validated<'ctx> {
    type Validated;

    fn assume_valid(&self) -> &Self::Validated {
        unsafe { &*(self as *const _ as *const _) }
    }
}

pub trait Validate<'a, 'ctx, V: Validator<'ctx>> : Validated<'ctx> {
    type ValidatePoll : ValidatePoll<'ctx, V>;

    fn init_validate(&'a self) -> Self::ValidatePoll;

}

pub trait ValidatePoll<'ctx, V: Validator<'ctx>> {
    type Error : 'static + Error;

    fn poll(&mut self, ctx: &mut V) -> Poll<Result<(), Self::Error>, V::Pending>;
}

impl<'ctx, V: Validator<'ctx>> ValidatePoll<'ctx, V> for () {
    type Error = !;
    fn poll(&mut self, _: &mut V) -> Poll<Result<(), Self::Error>, V::Pending> {
        Poll::Ready(Ok(()))
    }
}

pub trait Validator<'ctx> {
    type Pending;
    type Error : 'static + Error;

    fn downcast_mut<'a, T: AnyRef<'a>>(&'a mut self) -> Option<&'a mut T>;
}

impl<'ctx, T: ?Sized + Validator<'ctx>> Validator<'ctx> for &'_ mut T {
    type Pending = T::Pending;
    type Error = T::Error;

    fn downcast_mut<'a, U: AnyRef<'a>>(&'a mut self) -> Option<&'a mut U> {
        (**self).downcast_mut::<U>()
    }
}

impl<'ctx, T: ?Sized + Validator<'ctx>> Validator<'ctx> for Box<T> {
    type Pending = T::Pending;
    type Error = T::Error;

    fn downcast_mut<'a, U: AnyRef<'a>>(&'a mut self) -> Option<&'a mut U> {
        (**self).downcast_mut::<U>()
    }
}

pub unsafe trait AnyRef<'a> : 'a {
    fn type_id() -> TypeId;
}

//#[cfg(test)]
pub mod tests {
    use super::*;

    use std::cell::Cell;
    use std::any::TypeId;

    use thiserror::Error;

    #[derive(Debug, Default)]
    pub struct Block<'ctx> {
        marker: PhantomData<fn(&'ctx mut ChainState) -> &'ctx mut ChainState>,
        height: usize,
    }

    impl Block<'static> {
        fn new(height: usize) -> Self {
            Self {
                marker: PhantomData,
                height,
            }
        }
    }

    unsafe impl<'a, 'ctx> Validated<'ctx> for Block<'_> {
        type Validated = Block<'ctx>;
    }

    impl<'a, 'b: 'a, 'ctx, V: Validator<'ctx>> Validate<'a, 'ctx, V> for Block<'b> {
        type ValidatePoll = &'a Self;

        fn init_validate(&'a self) -> Self::ValidatePoll {
            self
        }
    }

    impl<'a, 'ctx, V: Validator<'ctx>> ValidatePoll<'ctx, V> for &'a Block<'_> {
        type Error = BlockError;

        fn poll(&mut self, ctx: &mut V) -> Poll<Result<(), Self::Error>, V::Pending> {
            match ctx.downcast_mut::<BlockValidator<'ctx>>() {
                None => Poll::Ready(Ok(())),
                Some(block_validator) => {
                    block_validator.validate(self)
                }
            }
        }
    }

    #[derive(Debug, Default)]
    pub struct ChainState {
        height: Cell<usize>,
    }

    impl ChainState {
        fn best_block<'ctx>(&'ctx self) -> Block<'ctx> {
            Block {
                marker: PhantomData,
                height: self.height.get(),
            }
        }
        fn increment(&self) {
            self.height.set(self.height.get() + 1);
        }

        fn invalidate(&mut self) {
            self.height = 0.into();
        }
    }

    #[derive(Debug, Error)]
    #[error("txout error")]
    pub struct BlockError;

    #[derive(Debug)]
    pub struct BlockValidator<'ctx> {
        state: &'ctx ChainState,
    }

    impl<'ctx> BlockValidator<'ctx> {
        fn validate<P>(&self, txout: &Block) -> Poll<Result<(), BlockError>, P> {
            if txout.height <= self.state.height.get() {
                Poll::Ready(Ok(()))
            } else {
                Poll::Ready(Err(BlockError))
            }
        }
    }

    unsafe impl<'a, 'ctx: 'a> AnyRef<'a> for BlockValidator<'ctx> {
        fn type_id() -> TypeId {
            TypeId::of::<BlockValidator>()
        }
    }

    impl<'ctx> Validator<'ctx> for BlockValidator<'ctx> {
        type Pending = !;
        type Error = BlockError;

        fn downcast_mut<'a, U: AnyRef<'a>>(&'a mut self) -> Option<&'a mut U> {
            if U::type_id() == BlockValidator::type_id() {
                Some(unsafe { &mut *(self as *mut _ as *mut _) })
            } else {
                None
            }
        }
    }

    pub fn test_blockval(block: &Block, ctx: &mut BlockValidator) -> Poll<Result<(), BlockError>, !> {
        let mut poll = Validate::<BlockValidator>::init_validate(block);
        poll.poll(ctx)
    }

    pub fn test_blockval_tuple(v: &(bool, Block, (Block, u8)), ctx: &mut BlockValidator)
        -> Poll<Result<(), super::impls::TupleError>, !>
    {
        let mut poll = Validate::<BlockValidator>::init_validate(v);
        poll.poll(ctx)
    }

    #[test]
    fn test() {
        let mut state = ChainState::default();
        let block0 = state.best_block();
        state.increment();
        let block1 = state.best_block();
        state.invalidate();

        let mut ctx = BlockValidator { state: &state };

        let value = (true, Block::new(0));
        let mut poll = dbg!(Validate::<'_, '_, BlockValidator>::init_validate(&value));
        dbg!(poll.poll(&mut ctx));

        let value = (true, Block::new(100));
        let mut poll = dbg!(Validate::<'_, '_, BlockValidator>::init_validate(&value));
        dbg!(poll.poll(&mut ctx));
    }
}

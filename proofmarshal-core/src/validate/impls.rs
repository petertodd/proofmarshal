use super::*;

use std::error::Error;
use thiserror::Error;

macro_rules! impl_validate_for_scalars {
    ($($t:ty,)+) => {$(
        unsafe impl Validated<'_> for $t {
            type Validated = Self;
        }

        impl<'ctx, V: Validator<'ctx>> Validate<'_, 'ctx, V> for $t {
            type ValidatePoll = ();
            fn init_validate(&self) -> () {}
        }
    )+}
}

impl_validate_for_scalars! {
    (), bool, char,
    f32, f64,
    u8, u16, u32, u64, u128, usize,
    i8, i16, i32, i64, i128, isize,
}

unsafe impl<'ctx, T: Validated<'ctx>> Validated<'ctx> for Box<T> {
    type Validated = Box<T::Validated>;
}

impl<'a, 'ctx, V: Validator<'ctx>, T> Validate<'a, 'ctx, V> for Box<T>
where T: Validate<'a, 'ctx, V>
{
    type ValidatePoll = T::ValidatePoll;

    fn init_validate(&'a self) -> Self::ValidatePoll {
        (**self).init_validate()
    }
}

unsafe impl<'ctx, T: Validated<'ctx>> Validated<'ctx> for Option<T> {
    type Validated = Option<T::Validated>;
}

impl<'a, 'ctx, V: Validator<'ctx>, T> Validate<'a, 'ctx, V> for Option<T>
where T: Validate<'a, 'ctx, V>
{
    type ValidatePoll = Option<T::ValidatePoll>;

    fn init_validate(&'a self) -> Self::ValidatePoll {
        self.as_ref().map(T::init_validate)
    }
}

impl<'ctx, V: Validator<'ctx>, T: ValidatePoll<'ctx, V>> ValidatePoll<'ctx, V> for Option<T> {
    type Error = T::Error;

    fn poll(&mut self, ctx: &mut V) -> Poll<Result<(), Self::Error>, V::Pending> {
        match self {
            None => Poll::Ready(Ok(())),
            Some(inner) => inner.poll(ctx),
        }
    }
}

#[derive(Debug)]
pub struct ArrayValidator<T> {
    state: Box<[T]>,
    idx: usize,
}

unsafe impl<'ctx, T: Validated<'ctx>, const N: usize> Validated<'ctx> for [T; N] {
    type Validated = [T::Validated; N];
}

impl<'a, 'ctx, V: Validator<'ctx>, T, const N: usize> Validate<'a, 'ctx, V> for [T; N]
where T: Validate<'a, 'ctx, V>
{
    type ValidatePoll = ArrayValidator<T::ValidatePoll>;

    fn init_validate(&'a self) -> Self::ValidatePoll {
        let mut state = vec![];
        for item in self.iter() {
            state.push(item.init_validate())
        }

        ArrayValidator {
            state: state.into_boxed_slice(),
            idx: 0
        }
    }
}

#[derive(Debug, Error)]
#[error("FIXME")]
pub struct ValidateArrayError<E: Error> {
    err: E,
    idx: usize,
}

impl<'ctx, V: Validator<'ctx>, T: ValidatePoll<'ctx, V>> ValidatePoll<'ctx, V> for ArrayValidator<T> {
    type Error = T::Error;

    fn poll(&mut self, ctx: &mut V) -> Poll<Result<(), Self::Error>, V::Pending> {
        while self.idx < self.state.len() {
            self.state[self.idx].poll(ctx)?;
            self.idx += 1;
        }
        Poll::Ready(Ok(()))
    }
}

#[derive(Debug, Error)]
#[error("FIXME")]
pub struct TupleError {
    err: Box<dyn Error + 'static>,
    idx: usize,
}

macro_rules! peel {
    ($name:ident, $( $rest_name:ident,)* ) => (tuple! { $( $rest_name, )* })
}

macro_rules! tuple {
    () => ();
    ( $($name:ident,)+ ) => {
        #[allow(non_snake_case)]
        unsafe impl<'ctx, $($name: Validated<'ctx>),+ > Validated<'ctx> for ($($name,)+) {
            type Validated = ( $(<$name as Validated<'ctx>>::Validated,)+ );
        }

        #[allow(non_snake_case)]
        impl<'a, 'ctx, V: Validator<'ctx>, $($name: Validate<'a, 'ctx, V>),+ > Validate<'a, 'ctx, V> for ($($name,)+) {
            type ValidatePoll = ( $(<$name as Validate<'a, 'ctx, V>>::ValidatePoll,)+ );

            fn init_validate(&'a self) -> Self::ValidatePoll {
                let ($(ref $name,)+) = self;
                ( $($name.init_validate(),)+ )
            }
        }

        #[allow(non_snake_case)]
        impl<'ctx, V: Validator<'ctx>, $($name: ValidatePoll<'ctx, V>),+ > ValidatePoll<'ctx, V> for ($($name,)+) {
            type Error = TupleError;

            fn poll(&mut self, ctx: &mut V) -> Poll<Result<(), Self::Error>, V::Pending> {
                let ($(ref mut $name,)+) = self;
                let mut idx = 0;
                $(
                    match $name.poll(ctx) {
                        Poll::Pending(p) => return Poll::Pending(p),
                        Poll::Ready(Err(err)) => return Poll::Ready(Err(
                            TupleError {
                                err: Box::new(err),
                                idx,
                            })
                        ),
                        Poll::Ready(Ok(())) => {},
                    };

                    idx += 1;
                )+
                drop(idx);
                Poll::Ready(Ok(()))
            }
        }
        peel! { $( $name, )+ }
    }
}

tuple! { T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, }

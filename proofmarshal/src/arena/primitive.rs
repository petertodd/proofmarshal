use crate::digest::Digest;

impl<T,A> Coerce<A> for Digest<T> {
    type Coerced = Self;
}

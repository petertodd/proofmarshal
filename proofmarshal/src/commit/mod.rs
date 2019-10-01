use crate::digest::Digest;

pub trait Commit {
    type Committed : 'static;
    fn commit(&self) -> Digest<Self::Committed>;
}

impl<T: 'static> Commit for Digest<T> {
    type Committed = Self;

    fn commit(&self) -> Digest<Self::Committed> {
        self.cast()
    }
}


macro_rules! impl_commit {
    ($( $t:ty, )+) => {
        $(
            impl Commit for $t {
                type Committed = $t;

                fn commit(&self) -> Digest<Self::Committed> {
                    Digest::hash_verbatim(self)
                }
            }
        )+
    }
}

impl_commit! {
    (), bool,
    u8, u16, u32, u64, u128,
    i8, i16, i32, i64, i128,
}

impl Commit for ! {
    type Committed = !;
    fn commit(&self) -> Digest<Self::Committed> {
        match *self {}
    }
}

#[cfg(test)]
mod tests {
}

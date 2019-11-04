use super::*;

pub mod pile;

pub trait Type<A> : Pointee + Owned {
    fn pile_layout(metadata: Self::Metadata) -> pile::Layout;
}

pub trait Load<A: Arena> : Type<A> {
    type Error : 'static + fmt::Debug;

    /// Load from a `Pile`.
    fn pile_load<'p, L>(loader: L, metadata: Self::Metadata) -> (Ref<'p, Self>, L::Done)
        where L: pile::Loader<'p, Arena=A>;
}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
    }
}

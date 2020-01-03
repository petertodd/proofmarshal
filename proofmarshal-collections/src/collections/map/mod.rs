use crate::prelude::*;

/// Key-value mapping.
pub struct Map<K: Fact<P>, V: Fact<P>, P=()> {
    tip: Own<Node<K,V,P,
}

/*
enum Node<K,V,P,S=()> {
    Empty,
    Value(T),
    Inner {
        left: Cache<Node<K,V,P>, P>,
        right: Cache<Node<K,V,P>, P>,
    },
}
*/

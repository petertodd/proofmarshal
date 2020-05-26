use super::*;

use std::ops::Deref;

use hoard::prelude::*;

#[test]
fn basics() {
    let pile = Pile::default();
    let lhs = Tree::new_leaf_in(1u8, pile);
    let rhs = dbg!(Tree::new_leaf_in(2u8, pile));
    let tip = lhs.try_join_in(rhs, pile).unwrap();

    dbg!(tip);
}

#[test]
fn get() {
    let pile = Pile::default();
    let lhs = Tree::new_leaf_in(1u8, pile);
    let rhs = dbg!(Tree::new_leaf_in(2u8, pile));
    let tip = lhs.try_join_in(rhs, pile).unwrap();

    assert_eq!(tip.get(0).as_deref(),
               Some(&1));
    assert_eq!(tip.get(1).as_deref(),
               Some(&2));
    assert_eq!(tip.get(2).as_deref(),
               None);
}

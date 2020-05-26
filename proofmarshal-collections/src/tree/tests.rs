use super::*;

use std::ops::Deref;

use hoard::prelude::*;

#[test]
fn basics() {
    let pile = Pile::default();
    let lhs = Tree::new_leaf_in(1u8, pile);
    let rhs = Tree::new_leaf_in(2u8, pile);
    let _tip = lhs.try_join_in(rhs, pile).unwrap();
}

#[test]
fn get() {
    let pile = Pile::default();
    let lhs = Tree::new_leaf_in(1u8, pile);
    let rhs = Tree::new_leaf_in(2u8, pile);
    let tip = lhs.try_join_in(rhs, pile).unwrap();

    assert_eq!(tip.get(0).as_deref(),
               Some(&1));
    assert_eq!(tip.get(1).as_deref(),
               Some(&2));
    assert_eq!(tip.get(2).as_deref(),
               None);
}

#[test]
fn try_from_iter() {
    let pile = Pile::default();

    let tip = Tree::try_from_iter_in(vec![1u8,2,3,4], pile).unwrap();
    assert_eq!(tip.len(), 4);

    let tip = Tree::try_from_iter_in(0u16 .. 256, pile).unwrap();

    for i in 0 .. 256 {
        assert_eq!(tip.get(i as usize).as_deref(), Some(&i));
    }
}

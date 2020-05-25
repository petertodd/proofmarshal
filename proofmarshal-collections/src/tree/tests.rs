use super::*;

use hoard::prelude::*;

#[test]
fn test() {
    let pile = Pile::default();
    let lhs = Tree::new_leaf_in(1u8, pile);
    let rhs = dbg!(Tree::new_leaf_in(2u8, pile));
    let tip = lhs.try_join_in(rhs, pile).unwrap();

    dbg!(tip);
}

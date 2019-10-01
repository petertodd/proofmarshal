pub struct Tree<T: Fact<P>, P = ()> {
    tip: Cache<Node<T>, P>,
}

pub enum Node<T: Fact<P>, P = ()> {
    Leaf(Cache<T,P>),
    Inner {
        left: Cache<Tree<T>, P>,
        right: Cache<Tree<T>, P>,
    }
}

use std::io;

use proofmarshal::prelude::*;
use proofmarshal::{
    fact::Maybe,
    digest::Digest,
};


use proofmarshal_derive::{Commit, Verbatim};

#[derive(Clone, Debug, Verbatim)]
struct Inner<P: Ptr = ()> {
    left:  Option<Maybe<Digest<Self>, P>>,
    right: Option<Maybe<Digest<Self>, P>>,
}

impl<P: Ptr> Commit for Inner<P> {
    type Committed = Inner;

    fn encode_commit_verbatim<W: io::Write>(&self, dst: W) -> Result<W, io::Error> {
        unimplemented!()
    }
}

/*
impl<P: Get> Prune for Inner<P> {
    fn prune(&mut self) {
        self.left.prune();
        self.right.prune();
    }

    fn fully_prune(&mut self) {
        self.left.fully_prune();
        self.right.fully_prune();
    }
}

#[test]
fn recursive_prune() {
    let a: Own<Inner<Heap>, Heap> = Own::new(Inner { left: None, right: None });
    let b: Own<Inner<Heap>, Heap> = dbg!(Own::new(Inner { left: None, right: None }));

    let ab: Maybe<Digest<Inner<Heap>>, Heap> = Maybe::new(Inner { left: Some(a.into()), right: Some(b.into()) });

    dbg!(ab);
}
*/

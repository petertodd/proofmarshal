use super::*;

impl<T, S, P: Ptr, Z> Verbatim for SumTreeDyn<T, S, P, Z>
where T: Commit,
      S: MerkleSum<T>,
{
    const LEN: usize = <Digest as Verbatim>::LEN +
                       <S as Verbatim>::LEN +
                       <Height as Verbatim>::LEN;

    fn encode_verbatim_in(&self, dst: &mut impl WriteVerbatim) {
        dst.write(&self.tip_digest());
        dst.write(&self.sum());
        dst.write(&self.height());
    }
}

impl<T, S, P: Ptr, Z> Verbatim for SumTree<T, S, P, Z>
where T: Commit,
      S: MerkleSum<T>,
{
    const LEN: usize = <SumTreeDyn<T, S, P, Z> as Verbatim>::LEN;

    fn encode_verbatim_in(&self, dst: &mut impl WriteVerbatim) {
        (**self).encode_verbatim_in(dst)
    }
}

impl<T, S, P: Ptr, Z> Commit for SumTreeDyn<T, S, P, Z>
where T: Commit,
      S: MerkleSum<T>,
{
    type Committed = SumTree<T::Committed, S::Committed>;
}

impl<T, S, P: Ptr, Z> Commit for SumTree<T, S, P, Z>
where T: Commit,
      S: MerkleSum<T>,
{
    type Committed = SumTree<T::Committed, S::Committed>;
}

impl<T, S, P: Ptr> Verbatim for InnerDyn<T, S, P>
where T: Commit,
      S: MerkleSum<T>,
{
    const LEN: usize = ((<Digest as Verbatim>::LEN + <S as Verbatim>::LEN) * 2)
                       + <Height as Verbatim>::LEN;

    fn encode_verbatim_in(&self, dst: &mut impl WriteVerbatim) {
        dst.write(&self.left().tip_digest());
        dst.write(&self.left().sum());
        dst.write(&self.right().tip_digest());
        dst.write(&self.right().sum());
        dst.write(&self.height());
    }
}

impl<T, S, P: Ptr> Verbatim for Inner<T, S, P>
where T: Commit,
      S: MerkleSum<T>,
{
    const LEN: usize = <InnerDyn<T, S, P> as Verbatim>::LEN;

    fn encode_verbatim_in(&self, dst: &mut impl WriteVerbatim) {
        (**self).encode_verbatim_in(dst)
    }
}

impl<T, S, P: Ptr> Commit for InnerDyn<T, S, P>
where T: Commit,
      S: MerkleSum<T>,
{
    type Committed = Inner<T::Committed, S::Committed>;
}

impl<T, S, P: Ptr> Commit for Inner<T, S, P>
where T: Commit,
      S: MerkleSum<T>,
{
    type Committed = Inner<T::Committed, S::Committed>;
}

#[cfg(test)]
mod tests {
    use super::*;

    use hoard::prelude::*;

    #[test]
    fn tree_commit() {
        let tip = Tree::try_from_iter_in(0u16 .. 256, Heap).unwrap();

        let digest: Digest<Tree<u16>> = tip.commit();
        assert_eq!("e98513b76e6d07fd65ab82e66dec33b0c7682dbb25c995f3e8c097e672bafd5a",
                   digest.to_string());
    }

    #[test]
    fn sumtree_commit() {
        let tip = SumTree::<u8, u8, _, _>::try_from_iter_in(0u8 .. 8, Heap).unwrap();

        let digest: Digest<SumTree<u8, u8>> = tip.commit();
        assert_eq!("78fe7e05aaa87ea272441b14ee895e3f3d9de927eac63930e60802a554dc09b8",
                   digest.to_string());

        assert_eq!(tip.sum(), 28);
    }

    #[test]
    fn tree_sumtree_commit() {
        let iter = (0u8 .. 4).map(|i| SumTree::<u8, u8, _, _>::new_leaf_in(i, Heap));
        let tip = Tree::try_from_iter_in(iter, Heap).unwrap();
        let digest: Digest<Tree<SumTree<u8, u8>>> = tip.commit();

        assert_eq!("6afa19f27bf722f684b78a0cd7f15e8256b3dd2ff9c928340627debcafe1081a",
                   digest.to_string());
    }
}

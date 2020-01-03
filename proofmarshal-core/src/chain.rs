pub struct Block<T, S=()> {
    value: Digest<T>,
    sum: S,
    len: u64,
    prev: BlockChain<T,S>,
}

pub struct BlockChain<T,S=()> {
    tip: Option<Digest<Block<T,S>>>,
}

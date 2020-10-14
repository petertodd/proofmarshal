use super::*;

impl<T: Commit, const N: usize> Commit for [T; N] {
    const VERBATIM_LEN: usize = T::VERBATIM_LEN * N;
    type Committed = [T::Committed; N];

    fn encode_verbatim(&self, dst: &mut impl WriteVerbatim) {
        for item in self.iter() {
            dst.write(item);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        /*
        assert_eq!([true, false, true, false].encode_verbatim(),
                   &[1,0,1,0]);

        assert_eq!([1u8; 100].encode_verbatim(),
                   &[1u8; 100][..]);
        */
    }
}

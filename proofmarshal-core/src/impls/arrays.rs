use super::*;

impl<T: Commit, const N: usize> Commit for [T; N] {
    type Committed = [T::Committed; N];
}

impl<T: Verbatim, const N: usize> Verbatim for [T; N] {
    const LEN: usize = T::LEN * N;

    fn encode_verbatim<W: WriteVerbatim>(&self, mut dst: W) -> Result<W, W::Error> {
        for item in self.iter() {
            dst = dst.write(item)?;
        }
        dst.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let a = [1,2,3,4,5u8];

        assert_eq!(a.encode_verbatim(vec![]).unwrap(),
                   &[1,2,3,4,5]);
    }
}

use super::*;

impl<T: Verbatim, const N: usize> Verbatim for [T; N] {
    const LEN: usize = T::LEN * N;

    fn encode_verbatim<W: WriteVerbatim>(&self, mut dst: W) -> Result<W, W::Error> {
        for item in self.iter() {
            dst = dst.write(item)?;
        }
        dst.finish()
    }
}

impl<T: Prune, const N: usize> Prune for [T; N] {
    fn prune(&mut self) {
        self.iter_mut().for_each(T::prune)
    }
    fn fully_prune(&mut self) {
        self.iter_mut().for_each(T::fully_prune)
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

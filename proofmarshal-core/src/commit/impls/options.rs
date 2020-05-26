use super::*;

impl<T: Verbatim> Verbatim for Option<T> {
    const LEN: usize = 1 + T::LEN;

    fn encode_verbatim_in(&self, dst: &mut impl WriteVerbatim) {
        match self {
            None => dst.write_zeros(Self::LEN),
            Some(value) => {
                dst.write_bytes(&[1]);
                dst.write(value);
            }
        }
    }
}

impl<T: Commit> Commit for Option<T> {
    type Committed = Option<T::Committed>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn option_unit() {
        let opt: Option<()> = None;
        assert_eq!(opt.encode_verbatim(), &[0]);
        assert_eq!(Some(()).encode_verbatim(), &[1]);
    }

    #[test]
    fn option_option() {
        let opt: Option<Option<()>> = None;
        assert_eq!(opt.encode_verbatim(), &[0, 0]);
        assert_eq!(Some(Some(())).encode_verbatim(), &[1, 1]);
    }
}

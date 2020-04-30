use super::*;

impl<T: Verbatim> Verbatim for [T] {
    const LEN: usize = {
        match T::LEN {
            0 => 0,
            x => x,
        }
    };

    fn encode_verbatim<W: WriteVerbatim>(&self, dst: W) -> Result<W, W::Error> {
        if T::LEN == 0 {
            Ok(dst)
        } else {
            use sha2::Digest as _;
            let mut hasher = sha2::Sha256::new();

            for item in self {
                hasher = hasher.write(item).unwrap();
            }
            todo!()
        }
    }
}

use super::*;

impl<T: Verbatim> Verbatim for [T] {
    const LEN: usize = 32;

    fn encode_verbatim_in(&self, dst: &mut impl WriteVerbatim) {
        let mut hasher = CommitHasher::new();
        for item in self.iter() {
            hasher.write(item);
        }
        let digest = hasher.finalize();
        dst.write(&digest);
    }
}

impl<T: Commit> Commit for [T] {
    type Committed = Vec<T>;
}

impl<T: Verbatim> Verbatim for Vec<T> {
    const LEN: usize = 32;
    fn encode_verbatim_in(&self, dst: &mut impl WriteVerbatim) {
        (**self).encode_verbatim_in(dst)
    }
}

impl<T: Commit> Commit for Vec<T> {
    type Committed = Vec<T>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slice_types() {
        let slice: &[u8] = &[1,2,3];
        let vec: Vec<u8> = vec![1,2,3];
        let boxed: Box<[u8]> = vec![1,2,3].into_boxed_slice();

        let slice_commit: Digest<Vec<u8>> = slice.commit();
        let vec_commit: Digest<Vec<u8>> = vec.commit();
        let box_commit: Digest<Vec<u8>> = boxed.commit();

        assert_eq!(slice_commit, vec_commit);
        assert_eq!(slice_commit, box_commit);
    }

    #[test]
    fn short_slice() {
        let slice: &[u8] = &[];

        assert_eq!(slice.encode_verbatim(),
                   &[227, 176, 196, 66, 152, 252, 28, 20, 154, 251, 244, 200, 153, 111, 185, 36, 39, 174, 65, 228, 100, 155, 147, 76, 164, 149, 153, 27, 120, 82, 184, 85]);
    }
}

use super::*;

/*
impl<T: Commit> CommitRef for [T] {
    type CommitmentDyn = [T::Commitment];

    fn encode_commitment_bytes_dyn<'a>(&self, dst: BytesUninit<'a, Self::CommitmentDyn>) -> Bytes<'a, Self::CommitmentDyn> {
        let mut dst = dst.write_struct();
        for item in self {
            dst = dst.write_field(&item.to_commitment());
        }
        dst.done()
    }

    fn commitment_metadata(&self) -> usize {
        self.len()
    }

    fn hash_commitment_dyn_with<H: Hasher>(&self, mut hasher: H) -> H::Output {
        for item in self {
            hasher.hash_commitment(item);
        }
        hasher.finish()
    }
}

impl<T: Commit> CommitRef for Vec<T> {
    const HASH_COMMITMENT_METADATA: bool = <[T] as CommitRef>::HASH_COMMITMENT_METADATA;
    type CommitmentDyn = [T::Commitment];

    fn commitment_metadata(&self) -> usize {
        self.len()
    }

    fn encode_commitment_bytes_dyn<'a>(&self, dst: BytesUninit<'a, Self::CommitmentDyn>) -> Bytes<'a, Self::CommitmentDyn> {
        (**self).encode_commitment_bytes_dyn(dst)
    }

    fn hash_commitment_dyn_with<H: Hasher>(&self, hasher: H) -> H::Output {
        (**self).hash_commitment_dyn_with(hasher)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn commit() {
        fn t<T: Commit>(v: Vec<T>, expected_digest: &[u8]) {
            let digest_vec = HashCommit::<[T::Commitment]>::new(&v).digest();
            assert_eq!(digest_vec.as_ref(), expected_digest);

            let slice: &[T] = &v[..];
            let digest_slice = HashCommit::<[T::Commitment]>::new(slice).digest();
            assert_eq!(digest_vec, digest_slice);
        }

        // FIXME: what should we do to commit to the length here?
        t::<()>(
            vec![],
            &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );

        t::<()>(
            vec![(); 32],
            &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );

        t::<()>(
            vec![(); 1000],
            &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );


        t::<()>(
            vec![(); 1000],
            &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );
    }
}
*/

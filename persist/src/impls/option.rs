use super::*;

impl<Z: Zone, T: Encode<Z>> Encode<Z> for Option<T> {
    const BLOB_LAYOUT: BlobLayout = {
        let r = [BlobLayout::new(1).extend(T::BLOB_LAYOUT),
                 T::BLOB_LAYOUT];
        r[T::BLOB_LAYOUT.has_niche() as usize]
    };

    type Encode = Option<T::Encode>;

    fn encode(self) -> Self::Encode {
        self.map(|value| value.encode())
    }
}

impl<E: EncodePoll> EncodePoll for Option<E> {
    type Zone = E::Zone;
    type Target = Option<E::Target>;

    fn poll<P>(&mut self, saver: &mut P) -> Poll<Result<(), P::Error>>
        where P: Saver<Zone = Self::Zone>
    {
        match self {
            None => Poll::Ready(Ok(())),
            Some(inner) => inner.poll(saver),
        }
    }

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
        match self {
            None => {
                if E::Target::BLOB_LAYOUT.has_niche() {
                    dst.write_bytes(&[0])?
                } else {
                    dst
                }.write_padding(E::Target::BLOB_LAYOUT.size())?
                 .done()
            },
            Some(v) => {
                if E::Target::BLOB_LAYOUT.has_niche() {
                    dst.write_bytes(&[0])?
                } else {
                    dst
                }.write(v)?
                 .done()
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout() {
        assert_eq!(<Option<u8> as Encode<!>>::BLOB_LAYOUT,
                   BlobLayout::new(2));
    }
}

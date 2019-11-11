use super::*;

impl<Z: Zone, T: Save<Z>> Save<Z> for Option<T> {
    const BLOB_LAYOUT: BlobLayout = {
        let r = [BlobLayout::new(1).extend(T::BLOB_LAYOUT),
                 T::BLOB_LAYOUT];
        r[T::BLOB_LAYOUT.has_niche() as usize]
    };

    type SavePoll = Option<T::SavePoll>;

    fn save_poll(this: impl Take<Self>) -> Self::SavePoll {
        this.take_sized()
            .map(|v| T::save_poll(v))
    }
}

impl<E: SavePoll> SavePoll for Option<E>
where E::Target: Sized,
{
    type Zone = E::Zone;
    type Target = Option<E::Target>;

    fn save_children<P>(&mut self, ptr_saver: &mut P) -> Poll<Result<(), P::Error>>
        where P: PtrSaver<Zone = Self::Zone>
    {
        match self {
            None => Ok(()).into(),
            Some(e) => e.save_children(ptr_saver),
        }
    }

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
        match self {
            None => {
                if !E::Target::BLOB_LAYOUT.has_niche() {
                    dst.write_bytes(&[0])?
                } else {
                    dst
                }.write_padding(E::Target::BLOB_LAYOUT.size())?
                 .done()
            },
            Some(v) => {
                if !E::Target::BLOB_LAYOUT.has_niche() {
                    dst.write_bytes(&[1])?
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
    }
}

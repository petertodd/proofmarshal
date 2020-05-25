use super::*;

#[derive(Debug)]
pub struct SliceEncoder<T> {
    inner: Box<[T]>,
    idx: usize,
}

impl<Q, R, T: Encode<Q, R>> Save<Q, R> for [T] {
    type SavePoll = SliceEncoder<T::EncodePoll>;

    fn init_save(&self, dst: &impl SavePtr<Source=Q, Target=R>) -> Self::SavePoll {
        let mut inner = Vec::with_capacity(self.len());
        for item in self {
            inner.push(item.init_save(dst))
        }
        SliceEncoder {
            inner: inner.into(),
            idx: 0,
        }
    }
}

impl<Q, R, T: SavePoll<Q, R>> SavePoll<Q, R> for SliceEncoder<T> {
    fn save_poll<D: SavePtr<Source=Q, Target=R>>(&mut self, mut dst: D) -> Result<D, D::Error> {
        while self.idx < self.inner.len() {
            dst = self.inner[self.idx].save_poll(dst)?;
            self.idx += 1;
        }
        Ok(dst)
    }
}

impl<T: EncodeBlob> SaveBlob for SliceEncoder<T> {
    fn save_blob<W: AllocBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
        assert_eq!(self.idx, self.inner.len());
        let blob_len = T::BLOB_LEN.checked_mul(self.inner.len())
                                  .expect("FIXME: overflow");
        let mut dst = dst.alloc_blob(blob_len)?;

        for item in self.inner.iter() {
            dst = dst.write(item)?;
        }
        dst.done()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::offset::ShallowDumper;

    #[test]
    fn test() {
        let slice: &[u8] = &[1,2,3,4];
        let (buf, _) = ShallowDumper::new(0).save(slice);
        assert_eq!(buf, &[1,2,3,4]);
    }
}

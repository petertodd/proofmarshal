use super::*;

use core::any::type_name;
use core::fmt;


impl<Z: Zone, T: Save<Z>> Save<Z> for [T] {
    const BLOB_LAYOUT: BlobLayout = BlobLayout::new(usize::max_value());

    #[inline(always)]
    fn blob_layout(len: SliceLen<T>) -> BlobLayout {
        assert!(T::BLOB_LAYOUT.inhabited);

        let size = T::BLOB_LAYOUT.size().checked_mul(len.get()).unwrap_or_else(|| {
            panic!("{} overflowed", type_name::<Self>())
        });

        if let Some(niche) = T::BLOB_LAYOUT.niche() {
            BlobLayout::with_niche(size, niche)
        } else {
            BlobLayout::new(size)
        }
    }

    type SavePoll = Vec<<T as Save<Z>>::SavePoll>;
    fn save_poll(this: impl Take<Self>) -> Self::SavePoll {
        let mut saver = vec![];
        for item in this.take_owned() {
            saver.push(T::save_poll(item));
        }
        saver
    }
}

impl<Z: Zone, T: SavePoll<Z>> SavePoll<Z> for Vec<T>
where T::Target: Sized,
{
    type Target = [<T as SavePoll<Z>>::Target];

    fn save_children<P>(&mut self, saver: &mut P) -> Poll<Result<(), P::Error>>
        where P: SavePtr<Z>
    {
        let mut all_done = true;
        for item in self {
            all_done &= item.save_children(saver)?.is_ready();
        }

        match all_done {
            true => Poll::Ready(Ok(())),
            false => Poll::Pending,
        }
    }

    fn encode_blob<W: WriteBlob>(&self, mut dst: W) -> Result<W::Done, W::Error> {
        for item in self {
            dst = dst.write(item)?;
        }
        dst.done()
    }

    #[inline(always)]
    fn metadata(&self) -> <Self::Target as Pointee>::Metadata {
        unsafe {
            SliceLen::new_unchecked(self.len())
        }
    }
}

pub struct SliceError<T: Load<Z>, Z: Zone> {
    err: T::Error,
    idx: usize,
}

impl<T: Load<Z>, Z: Zone> fmt::Debug for SliceError<T,Z>
where T::Error: fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("err", &self.err)
            .field("idx", &self.idx)
            .finish()
    }
}

impl<Z: Zone, T: Load<Z>> Load<Z> for [T] {
    type Error = SliceError<T,Z>;

    type ValidateChildren = Vec<T::ValidateChildren>;

    fn validate_blob<'p>(blob: Blob<'p, Self, Z>) -> Result<ValidateBlob<'p, Self, Z>, Self::Error> {
        let len = blob.metadata().get();

        let mut blob = blob.validate();

        let mut state = vec![];
        for idx in 0 .. len {
            let child_validator = blob.field::<T>().map_err(|err| SliceError { err, idx })?;
            state.push(child_validator);
        }
        Ok(blob.done(state))
    }

    fn decode_blob<'p>(blob: FullyValidBlob<'p, Self, Z>, loader: &impl Loader<Z>) -> Vec<T> {
        let len = blob.metadata().get();
        let mut this = Vec::with_capacity(len);

        let mut blob = blob.decode_struct(loader);
        for _ in 0 .. len {
            let item = blob.field::<T>();
            this.push(item);
        }
        this
    }
}

impl<Z: Zone, T: ValidateChildren<Z>> ValidateChildren<Z> for Vec<T> {
    fn validate_children<V>(&mut self, v: &mut V) -> Poll<Result<(), V::Error>>
        where V: ValidatePtr<Z>
    {
        let mut all_done = true;

        for item in self {
            all_done &= item.validate_children(v)?.is_ready();
        }

        match all_done {
            true => Poll::Ready(Ok(())),
            false => Poll::Pending,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn encode_vec<T: Save<!>>(v: Vec<T>) -> Vec<u8> {
        let layout = <[T]>::blob_layout(Pointee::metadata(&v[..]));
        let mut dst = vec![0; layout.size()];

        let mut saver = <[T]>::save_poll(v);
        match saver.save_children(&mut ()) {
            Poll::Ready(Ok(())) => {},
            _ => panic!(),
        }

        saver.encode_blob(&mut dst[..]).unwrap();
        dst
    }

    fn try_decode_vec<T: Load<!>>(buf: &[u8], len: usize) -> Result<Ref<[T]>, SliceError<T, !>> {
        let metadata = SliceLen::new(len).unwrap();
        let blob = Blob::new(buf, metadata).unwrap();

        let mut validator = <[T]>::validate_blob(blob)?;
        match validator.poll(&mut ()) {
            Poll::Ready(Ok(fully_valid_blob)) => Ok(<[T]>::load_blob(fully_valid_blob, &mut ())),
            _ => panic!(),
        }
    }

    #[test]
    fn test() {
        assert_eq!(encode_vec(vec![1u8,2,3]),
                   &[1,2,3]);

        assert_eq!(encode_vec(vec![1u16,2,3]),
                   &[1,0,2,0,3,0]);

        assert_eq!(try_decode_vec::<u8>(&[1,2,3], 3).unwrap(),
                   Ref::<[u8]>::Owned(vec![1,2,3]));

        assert_eq!(try_decode_vec::<u16>(&[1,0,2,0,3,0], 3).unwrap(),
                   Ref::<[u16]>::Owned(vec![1,2,3]));
    }
}

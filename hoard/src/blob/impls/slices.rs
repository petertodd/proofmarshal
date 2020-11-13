use thiserror::Error;

use super::*;

use crate::pointee::SliceLayoutError;

#[derive(Debug, Error)]
#[error("FIXME")]
pub struct DecodeSliceBytesError<E: std::error::Error> {
    idx: usize,
    err: E,
}

unsafe impl<T: Blob> BlobDyn for [T] {
    type DecodeBytesError = DecodeSliceBytesError<T::DecodeBytesError>;

    fn try_size(len: usize) -> Result<usize, Self::LayoutError> {
        T::SIZE.checked_mul(len).and_then(|size|
            // FIXME: is this limit really reasonable?
            if size <= isize::MAX as usize {
                Some(size)
            } else {
                None
            }
        ).ok_or(SliceLayoutError)
    }

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        let mut dst = dst.write_struct();
        for item in self {
            dst = dst.write_field(item);
        }
        dst.done()
    }

    fn decode_bytes(blob: Bytes<'_, Self>) -> Result<MaybeValid<Self::Owned>, Self::DecodeBytesError> {
        // FIXME: what exactly is the story with an oversized slice?
        let len: usize = blob.metadata();
        let mut r = Vec::<T>::with_capacity(len);

        let mut items = blob.struct_fields();
        for idx in 0 .. len {
            let item = items.trust_field().map_err(|err| DecodeSliceBytesError { idx, err })?;
            r.push(item);
        }
        items.assert_done();

        Ok(r.into())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::prelude::*;

    #[test]
    fn try_size() {
        // Obvious case
        assert_eq!(<[u8] as BlobDyn>::try_size(0), Ok(0));
        assert_eq!(<[u8] as BlobDyn>::try_size(1), Ok(1));
        assert_eq!(<[u8] as BlobDyn>::try_size(isize::MAX as usize), Ok(isize::MAX as usize));

        // Errors in the obvious case
        assert_eq!(<[u8] as BlobDyn>::try_size(isize::MAX as usize + 1), Err(SliceLayoutError));
        assert_eq!(<[u8] as BlobDyn>::try_size(usize::MAX), Err(SliceLayoutError));
    }

    #[test]
    fn encode_decode_roundtrip() {
        #[track_caller]
        fn t<T: Blob + Eq + std::fmt::Debug>(src: &[T], expected_bytes: &[u8]) {
            let actual_bytes = src.to_blob_bytes_dyn();
            assert_eq!(actual_bytes, expected_bytes);

            let actual_bytes = Bytes::try_from_slice(&actual_bytes, src.len()).unwrap();
            let round_trip: Vec<T> = <[T] as BlobDyn>::decode_bytes(actual_bytes).ok().unwrap().trust();

            assert_eq!(round_trip, src);
        }

        t(&[();0], &[]);
        t(&[();1000], &[]);

        t(&[1u8, 2, 3, 4, 5],
          &[1,2,3,4,5]);

        t(&[true, false, true, false],
          &[1, 0, 1, 0]);

        t(&[0x1234, 0x5678, 0x9abc, 0xdef0_u16],
          &[0x34, 0x12, 0x78, 0x56, 0xbc, 0x9a, 0xf0, 0xde]);
    }

    #[test]
    fn decode_err() {
        #[track_caller]
        fn t<T: Blob>(src: &[u8], expected_idx: usize, expected_err: T::DecodeBytesError)
            where T::DecodeBytesError: Eq
        {
            let len = src.len() / T::SIZE;
            let src = Bytes::try_from_slice(&src, len).unwrap();
            if let Err(err) = <[T] as BlobDyn>::decode_bytes(src) {
                assert_eq!(err.idx, expected_idx);
                assert_eq!(err.err, expected_err);
            } else {
                panic!()
            }
        }

        use crate::primitive::impls::DecodeBoolError;
        t::<bool>(&[3], 0, DecodeBoolError);
        t::<bool>(&[0,3], 1, DecodeBoolError);
        t::<bool>(&[0,3,1], 1, DecodeBoolError);
    }
}

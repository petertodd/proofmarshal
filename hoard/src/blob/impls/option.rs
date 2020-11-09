use super::*;

#[derive(Error, Debug)]
#[error("FIXME")]
pub enum DecodeOptionBytesError<E: std::fmt::Debug> {
    Discriminant,
    Padding,
    Value(E),
}

impl<T: Blob> Blob for Option<T> {
    const SIZE: usize = 1 + T::SIZE;

    type DecodeBytesError = DecodeOptionBytesError<T::DecodeBytesError>;

    fn decode_bytes(src: Bytes<'_, Self>) -> Result<MaybeValid<Self>, Self::DecodeBytesError> {
        let mut fields = src.struct_fields();

        match fields.trust_field::<u8>().into_ok() {
            0 => {
                // FIXME: check padding
                Ok(MaybeValid::from(None))
            },
            1 => {
                let inner = fields.trust_field::<T>().map_err(DecodeOptionBytesError::Value)?;
                Ok(MaybeValid::from(Some(inner)))
            },
            _ => Err(DecodeOptionBytesError::Discriminant),
        }
    }

    fn encode_bytes<'a>(&self, dst: BytesUninit<'a, Self>) -> Bytes<'a, Self> {
        let dst = dst.write_struct();
        match self {
            None => {
                dst.write_field(&0u8)
                   .write_padding(T::SIZE)
                   .done()
            },
            Some(inner) => {
                dst.write_field(&1u8)
                   .write_field(inner)
                   .done()
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let opt: Option<u8> = Some(23);
        assert_eq!(opt.to_blob_bytes(), &[1,23]);
    }
}

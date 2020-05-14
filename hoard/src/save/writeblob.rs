pub trait WriteBlob : Sized {
    type Ok;
    type Error;

    fn write_bytes(self, src: &[u8]) -> Result<Self, Self::Error>;
    fn finish(self) -> Result<Self::Ok, Self::Error>;

    /// Writes padding bytes.
    #[inline(always)]
    fn write_padding(mut self, len: usize) -> Result<Self, Self::Error> {
        for i in 0 .. len {
            self = self.write_bytes(&[0])?;
        }
        Ok(self)
    }


    /*
    #[inline(always)]
    fn write<'a, Q, T: super::Encode<'a, Q>>(self, value: &'a T, state: &T::State) -> Result<Self, Self::Error> {
        /*
        value.encode_blob(
            state,
            FieldWriter::new(self, mem::size_of::<T::Encoded>()),
        )
        */ todo!()
    }
    */

/*
    #[inline(always)]
    fn write_primitive<'a, T: Encode<'a, !>>(self, value: &'a T) -> Result<Self, Self::Error> {
        let state = value.make_encode_state();
        self.write(value, &state)
    }
*/
}

impl WriteBlob for ! {
    type Ok = !;
    type Error = !;

    fn write_bytes(self, _src: &[u8]) -> Result<Self, Self::Error> {
        match self {}
    }

    fn finish(self) -> Result<Self::Ok, Self::Error> {
        match self {}
    }
}


impl WriteBlob for Vec<u8> {
    type Ok = Self;
    type Error = !;

    fn write_bytes(mut self, src: &[u8]) -> Result<Self, Self::Error> {
        self.extend_from_slice(src);
        Ok(self)
    }

    fn finish(self) -> Result<Self::Ok, Self::Error> {
        Ok(self)
    }
}


#[cfg(test)]
mod tests {
    #[test]
    fn test() {
    }
}

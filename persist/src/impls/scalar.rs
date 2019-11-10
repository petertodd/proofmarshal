use super::*;

use core::marker::PhantomData;
use core::mem;
use core::slice;

#[derive(Debug)]
pub struct ScalarEncoder<T, Z> {
    marker: PhantomData<fn(Z) -> Z>,
    pub value: T,
}

impl<T, Z> From<T> for ScalarEncoder<T, Z> {
    fn from(value: T) -> Self {
        ScalarEncoder { marker: PhantomData, value }
    }
}

impl<Z: Zone> Encode<Z> for ! {
    const BLOB_LAYOUT: BlobLayout = BlobLayout::never();

    type Encode = ScalarEncoder<Self, Z>;
    fn encode(self) -> Self::Encode {
        self
    }
}

impl<Z: Zone> EncodePoll for ScalarEncoder<!, Z> {
    type Zone = Z;
    type Target = !;

    fn encode_blob<W: WriteBlob>(&self, _dst: W) -> Result<W::Done, W::Error> {
        match self.value {}
    }
}

impl<Z: Zone> Encode<Z> for () {
    const BLOB_LAYOUT: BlobLayout = BlobLayout::new(0);

    type Encode = ScalarEncoder<Self, Z>;
    fn encode(self) -> Self::Encode {
        self.into()
    }
}

impl<Z: Zone> EncodePoll for ScalarEncoder<(), Z> {
    type Zone = Z;
    type Target = ();

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
        dst.done()
    }
}

impl<Z: Zone> Encode<Z> for bool {
    const BLOB_LAYOUT: BlobLayout = BlobLayout::new(1);

    type Encode = ScalarEncoder<Self, Z>;
    fn encode(self) -> Self::Encode {
        self.into()
    }
}

impl<Z: Zone> EncodePoll for ScalarEncoder<bool, Z> {
    type Zone = Z;
    type Target = bool;

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
        dst.write_bytes(&[self.value as u8])?
           .done()
    }
}

macro_rules! impl_ints {
    ($( $t:ty, )+) => {
        $(
            impl<Z: Zone> Encode<Z> for $t {
                const BLOB_LAYOUT: BlobLayout = BlobLayout::new(mem::size_of::<Self>());

                type Encode = ScalarEncoder<Self, Z>;
                fn encode(self) -> Self::Encode {
                    self.into()
                }
            }

            impl<Z: Zone> EncodePoll for ScalarEncoder<$t, Z> {
                type Zone = Z;
                type Target = $t;

                fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
                    dst.write_bytes(&self.value.to_le_bytes())?
                       .done()
                }
            }
        )+
    }
}

impl_ints! {
    u8, u16, u32, u64, u128,
    i8, i16, i32, i64, i128,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ints() {
        let buf = [0u8;8];
        let e = Encode::<!>::encode(42u8);
    }
}

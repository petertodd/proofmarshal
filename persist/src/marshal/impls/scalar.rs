use super::*;

use core::marker::PhantomData;
use core::mem;
use core::slice;
use core::num::{
    NonZeroU8, NonZeroU16, NonZeroU32, NonZeroU64,
    NonZeroI8, NonZeroI16, NonZeroI32, NonZeroI64,
};

impl<Z: Zone> Save<Z> for ! {
    const BLOB_LAYOUT: BlobLayout = BlobLayout::never();

    type SavePoll = SaveScalar<Self, Z>;

    fn save_poll(this: impl Take<Self>) -> Self::SavePoll {
        match this.take_sized() {}
    }
}

impl<Z: Zone> SavePoll for SaveScalar<!, Z> {
    type Zone = Z;
    type Target = !;

    fn encode_blob<W: WriteBlob>(&self, _dst: W) -> Result<W::Done, W::Error> {
        match self.value {}
    }
}

impl<Z: Zone> Load<Z> for ! {
}


impl<Z: Zone> Save<Z> for () {
    const BLOB_LAYOUT: BlobLayout = BlobLayout::new(mem::size_of::<Self>());

    type SavePoll = SaveScalar<Self, Z>;

    fn save_poll(this: impl Take<Self>) -> Self::SavePoll {
        SaveScalar::new(this)
    }
}

impl<Z: Zone> SavePoll for SaveScalar<(), Z> {
    type Zone = Z;
    type Target = ();

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
        dst.done()
    }
}

impl<Z: Zone> Load<Z> for () {
}

#[derive(Debug)]
pub struct SaveScalar<T, Z> {
    marker: PhantomData<fn(Z) -> Z>,
    pub(crate) value: T,
}

impl<T,Z> SaveScalar<T,Z> {
    pub(crate) fn new(value: impl Take<T>) -> Self {
        SaveScalar {
            marker: PhantomData,
            value: value.take_sized(),
        }
    }
}

macro_rules! impl_ints {
    ($( $t:ty, )+) => {
        $(
            impl<Z: Zone> Save<Z> for $t {
                const BLOB_LAYOUT: BlobLayout = BlobLayout::new(mem::size_of::<Self>());
                type SavePoll = SaveScalar<Self, Z>;

                fn save_poll(this: impl Take<Self>) -> Self::SavePoll {
                    SaveScalar::new(this)
                }
            }

            impl<Z: Zone> SavePoll for SaveScalar<$t, Z> {
                type Zone = Z;
                type Target = $t;

                fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
                    dst.write_bytes(&self.value.to_le_bytes())?
                       .done()
                }
            }

            impl<Z: Zone> Load<Z> for $t {
            }
        )+
    }
}

impl_ints! {
    u8, u16, u32, u64, u128,
    i8, i16, i32, i64, i128,
}

/*
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


macro_rules! impl_nonzero_ints {
    ($( $t:ty, )+) => {
        $(
            impl<Z: Zone> Encode<Z> for $t {
                const BLOB_LAYOUT: BlobLayout = BlobLayout::new_nonzero(mem::size_of::<Self>());

                type Encode = ScalarEncoder<Self, Z>;
                fn encode(self) -> Self::Encode {
                    self.into()
                }
            }

            impl<Z: Zone> EncodePoll for ScalarEncoder<$t, Z> {
                type Zone = Z;
                type Target = $t;

                fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Done, W::Error> {
                    dst.write_bytes(&self.value.get().to_le_bytes())?
                       .done()
                }
            }
        )+
    }
}

impl_nonzero_ints! {
    NonZeroU8, NonZeroU16, NonZeroU32, NonZeroU64,
    NonZeroI8, NonZeroI16, NonZeroI32, NonZeroI64,
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
*/

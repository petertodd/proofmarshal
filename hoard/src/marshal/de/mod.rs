use super::*;

pub trait Decoder {
    type Zone;

    type Done;
    type Error;

    type DecodeTuple : DecodeTuple<Zone = Self::Zone, Done = Self::Done, Error = Self::Error>;
    fn decode_tuple(self) -> Result<Self::DecodeTuple, Self::Error>;

    fn decode_rec<T: ?Sized + Pointee, Y: Zone>(self) -> Result<(Self::Done, Rec<T,Y>), Self::Error>
        where Y: Decode<Self::Zone>,
              T: Load<Self::Zone>;

}

pub trait DecodeTuple : Sized {
    type Zone;

    type Done;
    type Error;

    fn decode_elem<T: Decode<Self::Zone>>(self) -> Result<(Self, T), Self::Error>;
    fn end(self) -> Result<Self::Done, Self::Error>;
}

impl Decoder for &'_ [u8] {
    type Zone = !;
    type Done = ();
    type Error = !;

    type DecodeTuple = Self;
    fn decode_tuple(self) -> Result<Self::DecodeTuple, Self::Error> {
        Ok(self)
    }

    fn decode_rec<T: ?Sized + Pointee, Y: Zone>(self) -> Result<(Self::Done, Rec<T,Y>), Self::Error> {
        todo!()
    }
}

impl DecodeTuple for &'_ [u8] {
    type Zone = !;
    type Done = ();
    type Error = !;

    fn decode_elem<T: Decode<Self::Zone>>(self) -> Result<(Self, T), Self::Error> {
        todo!()
    }

    fn end(self) -> Result<Self::Done, Self::Error> {
        todo!()
    }
}

use super::Blob;

pub trait Encoder {
    type Ok;
    type Error;

    type EncodeStruct : EncodeStruct<Ok=Self::Ok, Error=Self::Error>;

    fn encode_unit(self) -> Result<Self::Ok, Self::Error>;
    fn encode_bool(self, v: bool) -> Result<Self::Ok, Self::Error>;
    fn encode_u8(self, v: bool) -> Result<Self::Ok, Self::Error>;

    fn encode_struct(self, name: &'static str) -> Result<Self::EncodeStruct, Self::Error>;
}

pub trait EncodeStruct {
    type Ok;
    type Error;

    fn encode_field<T: Blob>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>;
    fn end(self) -> Result<Self::Ok, Self::Error>;
}

impl Encoder for Vec<u8> {
    type Ok = Self;
    type Error = !;

    type EncodeStruct = Self;

    fn encode_unit(self) -> Result<Self::Ok, Self::Error> {
        Ok(self)
    }

    fn encode_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_u8(self, v: bool) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_struct(self, name: &'static str) -> Result<Self::EncodeStruct, Self::Error> {
        todo!()
    }
}

impl EncodeStruct for Vec<u8> {
    type Ok = Self;
    type Error = !;

    fn encode_field<T: Blob>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error> {
        todo!()
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        todo!()
    }
}

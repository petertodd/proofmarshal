//! Efficient, append-only, persistence.

use super::*;
use marshal::Marshal;

mod layout;
pub use self::layout::Layout;

pub trait Pile : Zone {
    const OFFSET_LAYOUT: Layout;
    type Offset : 'static + Marshal<Self>;

    fn get_offset(ptr: &Self::Ptr) -> &Self::Offset;

    fn get_blob<'p>(&self, ptr: &'p Self::Ptr, size: usize) -> Result<&'p [u8], Self::Error>;
}

pub trait Dumper : Sized {
    type Pile : Pile;
    type Error;
    type Done;

    fn dump_blob(self, buf: &[u8]) -> Result<Self::Done, Self::Error>;

    fn dump_rec<T: ?Sized + Pointee, Z: Zone>(self, rec: &Rec<T,Z>) -> Result<(Self, <Self::Pile as Pile>::Offset), Self::Error>
    where T: Store<Self::Pile>,
          Z: Marshal<Self::Pile>;
}

impl Pile for ! {
    const OFFSET_LAYOUT: Layout = Layout::new(0);
    type Offset = ();

    fn get_offset(ptr: &!) -> &Self::Offset {
        match *ptr {}
    }

    fn get_blob<'p>(&self, _ptr: &'p Self::Ptr, _size: usize) -> Result<&'p [u8], Self::Error> {
        match *self {}
    }
}

impl Dumper for Vec<u8> {
    type Pile = !;
    type Error = !;
    type Done = Self;

    fn dump_rec<T: ?Sized + Pointee, Z: Zone>(self, rec: &Rec<T,Z>)
        -> Result<(Self, ()), Self::Error>
    {
        unreachable!()
    }

    fn dump_blob(mut self, buf: &[u8]) -> Result<Self::Done, Self::Error> {
        self.extend_from_slice(buf);
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
    }
}

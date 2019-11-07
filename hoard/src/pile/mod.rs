//! Efficient, append-only, persistence.

use super::*;

mod layout;
pub use self::layout::Layout;

pub trait Pile : Zone {
    const OFFSET_LAYOUT: Layout;
    type Offset : 'static + super::marshal::Marshal<Self>;

    fn get_blob<'p>(&self, ptr: &'p Self::Ptr, size: usize) -> Result<&'p [u8], Self::Error>;
}

pub trait Dumper : Sized {
    type Pile : Pile;
    type Error;
    type Done;

    fn dump_rec<T: ?Sized + Store<Self::Pile>>(self, rec: &Rec<T, Self::Pile>)
        -> Result<(Self, <Self::Pile as Pile>::Offset), Self::Error>;
    fn dump_blob(self, buf: &[u8]) -> Result<Self::Done, Self::Error>;
}

impl Pile for ! {
    const OFFSET_LAYOUT: Layout = Layout::new(0);
    type Offset = ();

    fn get_blob<'p>(&self, _ptr: &'p Self::Ptr, _size: usize) -> Result<&'p [u8], Self::Error> {
        match *self {}
    }
}

impl Dumper for Vec<u8> {
    type Pile = !;
    type Error = !;
    type Done = Self;

    fn dump_rec<T: ?Sized + Store<!>>(self, rec: &Rec<T,!>) -> Result<(Self, ()), Self::Error> {
        match rec.ptr().raw {}
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

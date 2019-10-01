use super::*;

#[derive(Debug)]
pub struct Missing;

#[derive(Debug)]
pub struct Error;

impl Arena for Missing {
    type Ptr = ();

    unsafe fn dealloc<T: ?Sized + Pointee>(_: (), _: T::Metadata) {}
}

impl Locate for Missing {
    type Error = Error;
    type Locator = Self;
}

impl Allocate for Missing {
    type Allocator = Self;

}

impl Alloc for Missing {
    type Arena = Self;

    fn locator(&self) -> &Self {
        self
    }

    fn alloc<T: Type<Self>>(&mut self, value: T) -> Own<T,Self> {
        unsafe {
            Own::from_raw((), value.ptr_metadata())
        }
    }
}


impl TryGet<Missing> for Missing {
    fn try_get<'p, T: ?Sized + Type<Missing>>(&self, _: &'p Own<T,Missing>) -> Result<&'p T, Error> {
        Err(Error)
    }
    fn try_take<T: Type<Missing>>(&self, _: Own<T,Missing>) -> Result<T, Error> {
        Err(Error)
    }
}

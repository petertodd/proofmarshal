use core::mem;

pub unsafe trait CastRef<T: ?Sized> {
}

pub unsafe trait CastMut<T: ?Sized> : CastRef<T> {
    fn cast_mut(&mut self) -> &mut T;
}

pub unsafe trait Cast<T> : CastRef<T> {
    fn cast(self) -> T;
}


pub unsafe trait TryCastRef<T: ?Sized> {
    type Error;
    fn try_cast_ref(&self) -> Result<&T, Self::Error>;
}

pub unsafe trait TryCastMut<T: ?Sized> : TryCastRef<T> {
    fn try_cast_mut(&mut self) -> Result<&mut T, Self::Error>;
}

pub unsafe trait TryCast<T> : TryCastRef<T> {
    fn try_cast(self) -> Result<T, Self::Error> where Self: Sized, T: Sized {
        assert_eq!(mem::size_of::<Self>(), mem::size_of::<T>());
        assert_eq!(mem::align_of::<Self>(), mem::align_of::<T>());

        match self.try_cast_ref() {
            Err(e) => Err(e),
            Ok(_) => {
                let this = mem::ManuallyDrop::new(self);
                unsafe { mem::transmute_copy(&this) }
            }
        }
    }
}

/*
unsafe impl<T: ?Sized, U: ?Sized> Cast<U> for T
where T: TryCast<U>,
      T::Error: Into<!>
{
    fn cast(self) -> U where Self: Sized, U: Sized {
        match self.try_cast() {
            Ok(r) => r,
            Err(e) => e.into(),
        }
    }

    fn cast_ref(&self) -> &U {
        match self.try_cast_ref() {
            Ok(r) => r,
            Err(e) => e.into(),
        }
    }
}

unsafe impl<T: ?Sized, U: ?Sized> CastMut<U> for T
where T: TryCastMut<U>,
      T::Error: Into<!>
{
    fn cast_mut(&mut self) -> &mut U {
        match self.try_cast_mut() {
            Ok(r) => r,
            Err(e) => e.into(),
        }
    }
}
*/

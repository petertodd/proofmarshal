use core::num;

use leint::Le;

use crate::zone::{Zone, FatPtr};

pub mod blob;

pub mod decode;
pub mod load;
use self::load::{PersistPointee, ValidatePointeeChildren};

pub trait PtrValidator<Z> {
    type Error;

    fn validate_ptr<'a, T>(&self, ptr: &'a FatPtr<T::Persist, Z::Persist>) -> Result<Option<&'a T::Persist>, Self::Error>
    where T: ?Sized + ValidatePointeeChildren<'a, Z>,
          Z: Zone;
}


#[cfg(test)]
mod tests {
    #[test]
    fn test() {
    }
}

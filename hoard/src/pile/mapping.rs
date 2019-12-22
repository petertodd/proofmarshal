use core::fmt;
use core::cmp;
use core::hash;

use super::error::DerefError;

pub unsafe trait Mapping : fmt::Debug {
    fn handle_deref_error<'p>(&'p self, err: DerefError<'p,'_>) -> ! {
        panic!("dereference failed: {:?}", err)
    }
}

unsafe impl Mapping for &'_ [u8] {
}

pub fn mapping_to_slice(mapping: &dyn Mapping) -> &&[u8] {
    unsafe {
        &*(mapping as *const dyn Mapping as *const &[u8])
    }
}

impl cmp::PartialEq<dyn Mapping + '_> for dyn Mapping + '_ {
    fn eq(&self, other: &dyn Mapping) -> bool {
        todo!()
    }
}
impl cmp::Eq for dyn Mapping + '_ {}

impl cmp::PartialOrd<dyn Mapping + '_> for dyn Mapping + '_ {
    fn partial_cmp(&self, other: &dyn Mapping) -> Option<cmp::Ordering> {
        todo!()
    }
}
impl cmp::Ord for dyn Mapping + '_ {
    fn cmp(&self, other: &dyn Mapping) -> cmp::Ordering {
        todo!()
    }
}

impl hash::Hash for dyn Mapping + '_ {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let orig_slice: &&[u8] = &&[1,2,3][..];
        let mapping: &dyn Mapping = orig_slice;

        let slice = mapping_to_slice(mapping);
        assert!(std::ptr::eq(orig_slice, slice));

        assert_eq!(format!("{:?}", mapping), "[1, 2, 3]");
    }
}

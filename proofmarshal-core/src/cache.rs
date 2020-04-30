use std::mem::MaybeUninit;
use std::sync::atomic::AtomicU8;

use hoard::prelude::*;

pub struct Cache<T, Z: Zone, U> {
    state: AtomicU8,
    fact: MaybeUninit<T>,
    evidence: MaybeUninit<OwnedPtr<U, Z>>,
}

impl<T, Z: Zone, U> Drop for Cache<T, Z, U> {
    fn drop(&mut self) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
    }
}

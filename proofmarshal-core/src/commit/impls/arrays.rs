use std::mem;

use super::*;

impl<T: Commit, const N: usize> Commit for [T; N] {
    type Commitment = [T::Commitment; N];

    fn to_commitment(&self) -> Self::Commitment {
        // FIXME: handle panics
        let r = MaybeUninit::<[_;N]>::uninit();
        let mut r: [MaybeUninit<T::Commitment>; N] = unsafe { r.assume_init() };
        for (item, dst) in self.iter().zip(r.iter_mut()) {
            let item_commitment = item.to_commitment();
            unsafe { dst.as_mut_ptr().write(item_commitment) }
        }
        unsafe { mem::transmute_copy(&r) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
    }
}

use super::*;

use std::mem;
use sliceinit::SliceInitializer;

impl<Z, T: Encoded<Z>, const N: usize> Encoded<Z> for [T; N] {
    type Encoded = [T::Encoded; N];
}

impl<'a, Z: Zone, T: Encode<'a, Z>, const N: usize> Encode<'a, Z> for [T; N] {
    type State = [T::State; N];

    fn save_children(&'a self) -> Self::State {
        let mut r: [MaybeUninit<T::State>; N] = unsafe { MaybeUninit::uninit().assume_init() };
        let mut initializer = SliceInitializer::new(&mut r[..]);

        for item in self.iter() {
            initializer.push(item.save_children());
        }
        initializer.done();

        let r2 = unsafe { mem::transmute_copy(&r) };
        assert_eq!(mem::size_of_val(&r), mem::size_of_val(&r2));
        assert_eq!(mem::align_of_val(&r), mem::align_of_val(&r2));
        r2
    }

    fn poll<D: Dumper<Z>>(&self, state: &mut Self::State, mut dumper: D) -> Result<D, D::Error> {
        for (item, state) in self.iter().zip(state.iter_mut()) {
            dumper = item.poll(state, dumper)?;
        }
        Ok(dumper)
    }

    fn encode_blob<W: WriteBlob>(&self, state: &[T::State; N], mut dst: W) -> Result<W::Ok, W::Error> {
        for (item, state) in self.iter().zip(state.iter()) {
            dst = dst.write(item, state)?;
        }
        dst.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::pile::PileMut;

    #[test]
    fn test() {
        let pile = PileMut::default();

        macro_rules! t {
            ($( $ar:expr => $expected:expr; )+) => {$({
                assert_eq!(pile.save_to_vec(&$ar),
                           &$expected);
            })+}
        }

        t! {
            [0u8; 0] => [];
            [42u8] => [42];
            [true, false, true] => [1,0,1];
            [[true, false, true]] => [1,0,1];
            [[1u8,2u8,3u8], [4u8,5u8,6u8]] => [1, 2, 3, 4, 5, 6];
        };
    }
}

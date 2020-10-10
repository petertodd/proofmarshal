use super::*;

impl<T: Load, const N: usize> Load for [T; N] {
    type Blob = [T::Blob; N];
    type Zone = T::Zone;

    fn load(_blob: Self::Blob, _zone: &Self::Zone) -> Self {
        todo!()
    }
}

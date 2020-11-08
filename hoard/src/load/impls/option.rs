use super::*;

impl<T: Load> Load for Option<T> {
    type Blob = Option<T::Blob>;
    type Ptr = T::Ptr;
    type Zone = T::Zone;

    fn load(blob: Self::Blob, zone: &Self::Zone) -> Self {
        match blob {
            None => None,
            Some(inner) => Some(T::load(inner, zone)),
        }
    }
}

use super::*;

impl<T: Load> Load for Option<T> {
    type Blob = Option<T::Blob>;
    type PtrClean = T::PtrClean;
    type Zone = T::Zone;

    fn load_maybe_valid(blob: MaybeValid<&Self::Blob>, zone: &Self::Zone) -> MaybeValid<Self> {
        match blob.trust().as_ref() {
            None => None,
            Some(inner) => Some(T::load(inner, zone)),
        }.into()
    }
}

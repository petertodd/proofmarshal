use super::*;

#[derive(Debug, PartialEq, Eq)]
pub struct ValidateSliceError<E> {
    idx: usize,
    err: E,
}

impl<E: ValidationError> ValidationError for ValidateSliceError<E> {}

impl<T: Validate> Validate for [T] {
    type Error = ValidateSliceError<T::Error>;

    fn validate<B: BlobValidator<Self>>(blob: B) -> Result<B::Ok, B::Error> {
        todo!()
    }
}

unsafe impl<Z: Zone, T: Load<Z>> Load<Z> for [T] {
    type ValidateChildren = Vec<T::ValidateChildren>;

    fn validate_children(&self) -> Self::ValidateChildren {
        todo!()
    }
}

impl<Z: Zone, T: ValidateChildren<Z>> ValidateChildren<Z> for Vec<T> {
    fn poll<V: PtrValidator<Z>>(&mut self, ptr_validator: &V) -> Result<(), V::Error> {
        loop {
            match self.last_mut() {
                Some(last) => {
                    last.poll(ptr_validator)?;
                    self.pop();
                },
                None => break Ok(()),
            }
        }
    }
}

#[cfg(test)]
mod test {
}

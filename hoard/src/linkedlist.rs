use core::marker::PhantomData;
use core::mem::{self, ManuallyDrop};
use core::ptr;

use crate::{*, marshal::{*, blob::*}};

#[derive(Debug)]
pub struct Cell<T, P: Ptr> {
    value: T,
    next: Option<OwnedPtr<Self, P>>,
}

impl<T, P: Ptr> Cell<T, P> {
    pub fn new(value: T, next: Option<OwnedPtr<Self, P>>) -> Self {
        Self { value, next }
    }

    pub fn value<'a>(self: Ref<'a, Self>) -> Ref<'a, T> {
        match self {
            Ref::Borrowed(this) => Ref::Borrowed(&this.value),
            Ref::Owned(this) => Ref::Owned(this.value),
        }
    }

    pub fn next<'a>(self: Ref<'a, Self>) -> Option<Ref<'a, OwnedPtr<Self, P>>> {
        match self {
            Ref::Borrowed(this) => {
                todo!()
            },
            Ref::Owned(this) => {
                todo!()
            },
        }
    }

    pub fn get<'a>(self: Ref<'a, Self>, mut n: usize, zone: &(impl Get<Ptr=P> + 'a)) -> Option<Ref<'a, T>> {
        let mut this = self;
        loop {
            if n == 0 {
                break Some(this.value())
            } else if let Some(next) = this.next() {
                n -= 1;
                this = zone.get_ref(next);
            } else {
                break None
            }
        }
    }

    pub fn push_front(&mut self, value: T, mut alloc: impl Alloc<Ptr=P>) {
        let old_value = mem::replace(&mut self.value, value);
        let next = Self {
            value: old_value,
            next: self.next.take(),
        };

        self.next = Some(alloc.alloc(next));
    }
}

impl<T, P: Ptr> Primitive for Cell<T, P> {
    type Error = !;

    const BLOB_LAYOUT: BlobLayout = BlobLayout::new(0);

    fn encode_blob<W: WriteBlob>(&self, dst: W) -> Result<W::Ok, W::Error> {
        todo!()
    }

    fn validate_blob<'p, Z: Zone>(blob: Blob<'p, Self, Z>) -> Result<FullyValidBlob<'p, Self, Z>, Self::Error> {
        todo!()
    }

    fn decode_blob<'p, Z: Zone>(blob: FullyValidBlob<'p, Self, Z>) -> Self {
        todo!()
    }
}

#[cfg(test)]
mod test {
}

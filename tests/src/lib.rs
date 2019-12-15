use hoard::pile::{Pile, PileMut, Offset, OffsetMut};
use hoard::{OwnedPtr, Get, GetMut, Ref, Alloc};

pub fn test_get<'a,'p,'v>(pile: &Pile<'p,'v>, ptr: &'a OwnedPtr<(Option<u8>, bool, bool), Offset<'p,'v>>)
-> Ref<'a, (Option<u8>, bool, bool)>
{
    pile.get(ptr)
}

pub fn test_mut_get<'a,'p,'v>(pile: &PileMut<'p,'v>, ptr: &'a OwnedPtr<(Option<u8>, bool, bool), OffsetMut<'p,'v>>)
-> Ref<'a, (Option<u8>, bool, bool)>
{
    pile.get(ptr)
}

pub fn test_get_mut<'a,'p,'v>(pile: &PileMut<'p,'v>, ptr: &'a mut OwnedPtr<(Option<u8>, bool, bool), OffsetMut<'p,'v>>)
-> &'a (Option<u8>, bool, bool)
{
    pile.get_mut(ptr)
}

pub fn test_alloc<'p,'v>(mut alloc: &mut PileMut<'p,'v>) -> OwnedPtr<Option<u8>, OffsetMut<'p, 'v>> {
    alloc.alloc(None)
}

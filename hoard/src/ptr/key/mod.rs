use std::error;

use super::*;

pub mod offset;
pub use self::offset::Offset;

pub mod map;
pub use self::map::Map;

#[derive(Debug)]
pub struct Key<'a, M: ?Sized, K = <M as Map>::Key> {
    key: K,
    map: &'a M,
}

impl<'a, M: ?Sized, K: Copy> Clone for Key<'a, M, K> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<'a, M: ?Sized, K: Copy> Copy for Key<'a, M, K> {}

impl<'a, M: ?Sized, K> From<!> for Key<'a, M, K> {
    fn from(never: !) -> Self {
        never
    }
}

impl<'a, M: ?Sized + Map> PtrClean for Key<'a, M> {
    type Zone = &'a M;
    type Blob = M::Key;

    fn zone(&self) -> Self::Zone {
        self.map
    }

    fn to_blob(self) -> Self::Blob {
        self.key
    }

    fn from_blob(key: Self::Blob, map: &Self::Zone) -> Self {
        Self {
            key,
            map: *map,
        }
    }
}

impl<'a, M: ?Sized + Map> TryGet for Key<'a, M> {
    type Error = Error<M::Id, M::Error>;

    unsafe fn try_get<T: ?Sized>(&self, metadata: T::Metadata) -> Result<MaybeValid<Ref<T>>, Self::Error>
        where T: LoadRefIn<Self::Zone>
    {
        self.map.get_blob_with(self.key, metadata, |bytes| {
            T::load_owned_from_bytes_in(bytes, &self.map)
        }).map_err(|err| Error::from_zone_error(self.map.id(), err))?
          .map_err(|err| Error::from_decode_error(self.map.id(), err))
          .map(|owned| {
              MaybeValid::from(Ref::Owned(owned.trust()))
          })
    }

    unsafe fn try_take_then<T: ?Sized, F, R>(self, metadata: T::Metadata, f: F) -> Result<R, Self::Error>
        where T: LoadRefIn<Self::Zone>,
              F: FnOnce(MaybeValid<RefOwn<T>>) -> R
    {
        todo!()
    }
}

impl<'a, M: ?Sized + Map> Get for Key<'a, M> {
    unsafe fn get<T: ?Sized>(&self, metadata: T::Metadata) -> MaybeValid<Ref<T>>
        where T: LoadRefIn<Self::Zone>
    {
        self.try_get::<T>(metadata)
            .expect("fixme")
    }

    unsafe fn take_then<T: ?Sized, F, R>(self, metadata: T::Metadata, f: F) -> R
        where T: LoadRefIn<Self::Zone>,
              F: FnOnce(MaybeValid<RefOwn<T>>) -> R
    {
        todo!()
    }
}

#[derive(Debug)]
pub enum KeyMut<'a, M: ?Sized, K = <M as Map>::Key> {
    Key(Key<'a, M, K>),
    Heap(Heap),
}

impl<M: ?Sized, K> From<!> for KeyMut<'_, M, K> {
    fn from(never: !) -> Self {
        never
    }
}

impl<'a, M: ?Sized, K> From<Key<'a, M, K>> for KeyMut<'a, M, K> {
    fn from(key: Key<'a, M, K>) -> Self {
        KeyMut::Key(key)
    }
}

impl<M: ?Sized, K> Default for KeyMut<'_, M, K> {
    fn default() -> Self {
        KeyMut::Heap(Heap::default())
    }
}

impl<'a, M: ?Sized + Map> Ptr for KeyMut<'a, M> {
    type Zone = &'a M;
    type Blob = M::Key;
    type Clean = Key<'a, M>;

    fn from_clean(key: Self::Clean) -> Self {
        KeyMut::Key(key)
    }

    unsafe fn dealloc<T: ?Sized + Pointee>(&mut self, metadata: T::Metadata) {
        match self {
            KeyMut::Key(_) => {},
            KeyMut::Heap(ptr) => ptr.dealloc::<T>(metadata),
        }
    }

    unsafe fn try_get_dirty<T: ?Sized + Pointee>(&self, metadata: T::Metadata) -> Result<MaybeValid<&T>, Self::Clean> {
        match self {
            KeyMut::Key(key) => Err(*key),
            KeyMut::Heap(ptr) => Ok(ptr.try_get_dirty::<T>(metadata).into_ok()),
        }
    }

    unsafe fn try_get_dirty_mut<T: ?Sized + Pointee>(&mut self, metadata: T::Metadata) -> Result<MaybeValid<&mut T>, Self::Clean> {
        match self {
            KeyMut::Key(key) => Err(*key),
            KeyMut::Heap(ptr) => Ok(ptr.try_get_dirty_mut::<T>(metadata).into_ok()),
        }
    }

    unsafe fn try_take_dirty_then<T: ?Sized + Pointee, F, R>(self, metadata: T::Metadata, f: F) -> Result<R, Self::Clean>
        where F: FnOnce(MaybeValid<RefOwn<T>>) -> R
    {
        todo!()
    }

    fn alloc<T: ?Sized + Pointee>(src: impl Take<T>) -> Bag<T, Self> {
        let bag: Bag<T, Heap> = Heap::alloc(src);
        let (ptr, metadata) = bag.into_raw_parts();
        unsafe {
            Bag::from_raw_parts(KeyMut::Heap(ptr), metadata)
        }
    }
}

impl<'a, M: ?Sized + Map> TryGet for KeyMut<'a, M> {
    type Error = Error<M::Id, M::Error>;

    unsafe fn try_get<T: ?Sized>(&self, metadata: T::Metadata) -> Result<MaybeValid<Ref<T>>, Self::Error>
        where T: LoadRefIn<Self::Zone>
    {
        match self {
            KeyMut::Key(key) => key.try_get::<T>(metadata),
            KeyMut::Heap(ptr) => Ok(ptr.try_get_dirty::<T>(metadata).into_ok().into()),
        }
    }

    unsafe fn try_take_then<T: ?Sized, F, R>(self, metadata: T::Metadata, f: F) -> Result<R, Self::Error>
        where T: LoadRefIn<Self::Zone>,
              F: FnOnce(MaybeValid<RefOwn<T>>) -> R
    {
        todo!()
    }
}

impl<'a, M: ?Sized + Map> Get for KeyMut<'a, M> {
    unsafe fn get<T: ?Sized>(&self, metadata: T::Metadata) -> MaybeValid<Ref<T>>
        where T: LoadRefIn<Self::Zone>
    {
        self.try_get::<T>(metadata)
            .expect("fixme")
    }

    unsafe fn take_then<T: ?Sized, F, R>(self, metadata: T::Metadata, f: F) -> R
        where T: LoadRefIn<Self::Zone>,
              F: FnOnce(MaybeValid<RefOwn<T>>) -> R
    {
        todo!()
    }
}


impl<'a, M: ?Sized + Map> KeyMut<'a, M> {
    unsafe fn try_make_dirty<T: ?Sized>(&mut self, key: Key<'a, M>, metadata: T::Metadata)
        -> Result<MaybeValid<&mut T>, Error<M::Id, M::Error>>
    where T: LoadRefIn<&'a M>
    {
        let owned: T::Owned = key.try_take::<T>(metadata)?.trust();
        let bag: Bag<T, Heap> = Heap::alloc(owned);
        let (ptr, _metadata) = bag.into_raw_parts();
        *self = Self::Heap(ptr);
        self.try_get_mut::<T>(metadata)
    }
}

impl<'a, M: ?Sized + Map> TryGetMut for KeyMut<'a, M> {
    unsafe fn try_get_mut<T: ?Sized>(&mut self, metadata: T::Metadata) -> Result<MaybeValid<&mut T>, Self::Error>
        where T: LoadRefIn<Self::Zone>
    {
        match *self {
            KeyMut::Heap(ref mut ptr) => Ok(ptr.try_get_dirty_mut::<T>(metadata).into_ok()),
            KeyMut::Key(key) => self.try_make_dirty::<T>(key, metadata),
        }
    }
}

impl<'a, M: ?Sized + Map> GetMut for KeyMut<'a, M> {
    unsafe fn get_mut<T: ?Sized>(&mut self, metadata: T::Metadata) -> MaybeValid<&mut T>
        where T: LoadRefIn<Self::Zone>
    {
        self.try_get_mut::<T>(metadata)
            .expect("fixme")
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn key() {
        let map: &[u8] = &[];

        let key = Key::from_blob(Offset::new(0), &map);
        let r = unsafe { key.try_get::<()>(()).unwrap().trust() };
        assert_eq!(r, &());

        let key = Key::from_blob(Offset::new(0), &map);
        let err = unsafe { key.try_get::<u8>(()).unwrap_err() };
        assert_eq!(err.zone_id(), &map.id());
        assert!(matches!(err.kind(), ErrorKind::Zone(_)));

        let map: &[u8] = &[0x12, 0x34, 0x56, 0x78];
        let key = Key::from_blob(Offset::new(0), &map);
        let r = unsafe { key.try_get::<u32>(()).unwrap().trust() };
        assert_eq!(r, &0x78563412);
    }

    #[test]
    fn keymut() {
        let bag: Bag<u8, KeyMut<[u8]>> = KeyMut::alloc(42u8);
        dbg!(bag.get());
    }
}

use super::*;

impl<T: SaveDirty, const N: usize> SaveDirty for [T; N] {
    type CleanPtr = T::CleanPtr;
    type SaveDirtyPoll = ArraySaveDirtyPoll<T::SaveDirtyPoll, N>;

    fn init_save_dirty(&self) -> Self::SaveDirtyPoll {
        todo!()
    }
}

pub struct ArraySaveDirtyPoll<T: SaveDirtyPoll, const N: usize> {
    idx: usize,
    polls: [T; N],
}

impl<T: SaveDirtyPoll, const N: usize> SaveDirtyPoll for ArraySaveDirtyPoll<T, N> {
    type CleanPtr = T::CleanPtr;
    type SavedBlob = [T::SavedBlob; N];

    fn save_dirty_poll_impl<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: BlobSaver<CleanPtr = Self::CleanPtr>
    {
        todo!()
    }

    fn encode_blob(&self) -> Self::SavedBlob {
        todo!()
    }
}

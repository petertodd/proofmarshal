use super::*;

impl<Y, Q: Ptr, T: Saved<Y, Q>> Saved<Y, Q> for Option<T> {
    type Saved = Option<T::Saved>;
}

impl<T: SaveDirty> SaveDirty for Option<T> {
    type CleanPtr = T::CleanPtr;
    type SaveDirtyPoll = OptionSaveDirtyPoll<T::SaveDirtyPoll>;

    fn init_save_dirty(&self) -> Self::SaveDirtyPoll {
        OptionSaveDirtyPoll(self.as_ref().map(|inner| inner.init_save_dirty()))
    }
}

#[derive(Debug)]
pub struct OptionSaveDirtyPoll<T>(Option<T>);

impl<T: SaveDirtyPoll> SaveDirtyPoll for OptionSaveDirtyPoll<T> {
    type CleanPtr = T::CleanPtr;
    type SavedBlob = Option<T::SavedBlob>;

    fn save_dirty_poll_impl<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: BlobSaver<CleanPtr = Self::CleanPtr>
    {
        match &mut self.0 {
            None => Ok(()),
            Some(inner) => inner.save_dirty_poll_impl(saver),
        }
    }

    fn encode_blob(&self) -> Self::SavedBlob {
        self.0.as_ref().map(T::encode_blob)
    }
}

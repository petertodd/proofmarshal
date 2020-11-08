use super::*;

impl<Q, T: Save<Q>> Save<Q> for Option<T> {
    type DstBlob = Option<T::DstBlob>;
    type SavePoll = OptionSavePoll<T::SavePoll>;

    fn init_save(&self) -> Self::SavePoll {
        OptionSavePoll(self.as_ref().map(|inner| inner.init_save()))
    }
}

#[derive(Debug)]
pub struct OptionSavePoll<T>(Option<T>);

impl<T: SavePoll> SavePoll for OptionSavePoll<T> {
    type SrcPtr = T::SrcPtr;
    type DstPtr = T::DstPtr;
    type DstBlob = Option<T::DstBlob>;

    fn save_poll<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: Saver<SrcPtr = Self::SrcPtr, DstPtr = Self::DstPtr>
    {
        match &mut self.0 {
            None => Ok(()),
            Some(inner) => inner.save_poll(saver),
        }
    }

    fn encode_blob(&self) -> Self::DstBlob {
        self.0.as_ref().map(T::encode_blob)
    }
}

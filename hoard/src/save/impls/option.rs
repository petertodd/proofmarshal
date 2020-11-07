use super::*;

impl<Q, R, T: Save<Q, R>> Save<Q, R> for Option<T> {
    type SrcBlob = Option<T::SrcBlob>;
    type DstBlob = Option<T::DstBlob>;
    type SavePoll = OptionSavePoll<T::SavePoll>;

    fn init_save(&self) -> Self::SavePoll {
        OptionSavePoll(self.as_ref().map(|inner| inner.init_save()))
    }

    fn init_save_from_blob(this: &Self::SrcBlob) -> Self::SavePoll {
        OptionSavePoll(this.as_ref().map(|inner| T::init_save_from_blob(inner)))
    }
}

#[derive(Debug)]
pub struct OptionSavePoll<T>(Option<T>);

impl<Q, R, T: SavePoll<Q, R>> SavePoll<Q, R> for OptionSavePoll<T> {
    type DstBlob = Option<T::DstBlob>;

    fn save_poll<S>(&mut self, saver: &mut S) -> Result<(), S::Error>
        where S: Saver<SrcPtr = Q, DstPtr = R>
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

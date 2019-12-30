use super::*;

pub trait ValidationError : Any + fmt::Debug + Send {
}

impl ValidationError for ! {
}

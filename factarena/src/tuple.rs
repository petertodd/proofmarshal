#[derive(Debug)]
#[repr(C)]
pub struct Item<T,N=()>(pub T,pub N);

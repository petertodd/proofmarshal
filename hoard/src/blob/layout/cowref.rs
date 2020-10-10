#[derive(Debug, Clone)]
pub enum CowRef<'a, T> {
    Ref(&'a T),
    Boxed(Box<T>),
}

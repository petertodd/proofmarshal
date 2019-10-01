use super::*;

#[derive(Debug, Clone, Copy)]
pub struct Enum<V>(pub V);

#[derive(Debug, Clone, Copy)]
pub enum Variant<T,N> {
    Match(T),
    Next(N),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
    }
}

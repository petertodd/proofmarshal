use crate::save::*;
use crate::load::*;

pub trait Primitive : Load + for<'a> Save<'a, !, !, Saved=Self> {
}

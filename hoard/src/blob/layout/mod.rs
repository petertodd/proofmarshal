use std::ops::Range;
use std::borrow::Cow;

mod cowref;
use self::cowref::CowRef;

#[derive(Debug, Clone)]
pub struct Layout {
    size: usize,
    nonzero_niche: Range<usize>,
    kind: Kind,
}

#[derive(Debug, Clone)]
pub enum Kind {
    Scalar(Scalar),
    Struct(Struct),
    Array(Array),
}

#[derive(Debug, Clone)]
pub struct Array {
    len: usize,
    item: &'static Layout,
}

#[derive(Debug, Clone)]
pub struct Struct {
    name: &'static str,
    fields: &'static [Field],
}

#[derive(Debug, Clone)]
pub struct Field {
    key: &'static str,
    layout: Layout,
}

#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum Scalar {
    Unit,
    Bool,
    U8,
}

/*
use self::ScalarKind::*;

impl ScalarKind {
    pub const fn size(&self) -> usize {
        match self {
            Unit => 0,
            Bool => 1,
            U8 => 1,
        }
    }

    pub const fn nonzero_niche(&self) -> Option<Range<usize>> {
        match self {
            Unit | Bool | U8 => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Field<'a> {
    key: &'a str,
    layout: &'a Layout<'a>,
}

impl<'a> Layout<'a> {
    pub const fn new(kind: Kind<'a>) -> Self {
        let (size, nonzero_niche) = match kind {
            Kind::Scalar(prim) => {
                (prim.size(),
                 match prim.nonzero_niche() {
                     Some(niche) => niche,
                     None => 0 .. 0,
                 }
                )
            },
            Kind::Struct { name: _, fields } => {
                let mut size = 0;
                let mut nonzero_niche = 0 .. 0;

                let mut i = 0;
                while i < fields.len() {
                    let field_layout = fields[i].layout;
                    size += field_layout.size;

                    if nonzero_niche.end - nonzero_niche.start
                        < field_layout.nonzero_niche.end - field_layout.nonzero_niche.start
                    {
                        nonzero_niche.start = field_layout.nonzero_niche.start;
                        nonzero_niche.end = field_layout.nonzero_niche.end;
                    }

                    i += 1;
                }

                (size, nonzero_niche)
            },
            Kind::Array { len, item } => {
                (item.size() * len,
                 match item.nonzero_niche() {
                     Some(niche) if len > 0 => niche,
                     _ => 0 .. 0,
                 }
                )
            },
        };

        Self {
            size,
            nonzero_niche,
            kind,
        }
    }

    pub const fn array(len: usize, item: &'a Layout<'a>) -> Self {
        Self::new(Kind::Array { len, item })
    }

    pub const fn size(&self) -> usize {
        self.size
    }

    pub const fn nonzero_niche(&self) -> Option<Range<usize>> {
        if self.nonzero_niche.start < self.nonzero_niche.end {
            Some(self.nonzero_niche.start .. self.nonzero_niche.end)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_struct_layout() {
        let empty = Layout::new(Kind::Struct {
            name: "foo",
            fields: &[],
        });

        assert_eq!(empty.size(), 0);
        assert_eq!(empty.nonzero_niche(), None);
    }
}
*/

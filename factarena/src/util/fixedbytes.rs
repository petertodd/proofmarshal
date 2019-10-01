pub trait WriteBytes : Sized {
    type Done;

    fn reserve<'a>(self, n: usize) -> Reservation<Self> {
        Reservation {
            orig: self,
            remaining: n,
        }
    }

    fn remaining(&self) -> usize;

    fn write(&mut self, buf: impl AsRef<[u8]>);
    fn done(self) -> Self::Done;

    fn write_all(mut self, buf: impl AsRef<[u8]>) -> Self::Done {
        self.write(buf);
        self.done()
    }
}

pub trait ReadBytes : Sized {
    type Done;

    fn reserve<'a>(self, n: usize) -> Reservation<Self> {
        Reservation {
            orig: self,
            remaining: n,
        }
    }

    fn remaining(&self) -> usize;

    fn read_all(&mut self, buf: &mut [u8]);

    fn done(self) -> Self::Done;
}

#[derive(Debug)]
pub struct Reservation<T> {
    orig: T,
    remaining: usize,
}

impl<W: WriteBytes> WriteBytes for Reservation<W> {
    type Done = W;

    #[inline(always)]
    fn write(&mut self, buf: impl AsRef<[u8]>) {
        let buf = buf.as_ref();
        self.remaining = self.remaining.checked_sub(buf.len())
                                        .expect("overflow");
        self.orig.write(buf)
    }

    #[inline(always)]
    fn remaining(&self) -> usize {
        self.remaining
    }

    #[inline(always)]
    fn done(self) -> Self::Done {
        assert_eq!(self.remaining, 0, "not all bytes written");
        self.orig
    }
}

impl<R: ReadBytes> ReadBytes for Reservation<R> {
    type Done = R;

    #[inline(always)]
    fn read_all(&mut self, buf: &mut [u8]) {
         self.remaining = self.remaining.checked_sub(buf.len())
                                        .expect("overflow");
         self.orig.read_all(buf)
    }

    #[inline(always)]
    fn remaining(&self) -> usize {
        self.remaining
    }

    #[inline(always)]
    fn done(self) -> Self::Done {
        assert_eq!(self.remaining, 0, "not all bytes read");
        self.orig
    }
}

impl ReadBytes for &'_ [u8] {
    type Done = ();

    #[inline(always)]
    fn read_all(&mut self, buf: &mut [u8]) {
        let (src, remainder) = self.split_at(buf.len());
        buf.copy_from_slice(src);
        *self = remainder;
    }

    #[inline(always)]
    fn remaining(&self) -> usize {
        self.len()
    }

    #[inline(always)]
    fn done(self) -> Self::Done {
        assert_eq!(self.len(), 0, "not all bytes read");
        ()
    }
}

#[derive(Debug)]
pub struct WriteCursor<'a> {
    buf: &'a mut [u8],
    remaining: usize,
}

impl<'a> WriteBytes for WriteCursor<'a> {
    type Done = &'a mut [u8];

    #[inline(always)]
    fn write(&mut self, buf: impl AsRef<[u8]>) {
        let buf = buf.as_ref();
        assert!(self.remaining >= buf.len(), "no remaining capacity");
        let written = self.buf.len() - self.remaining;

        self.buf[written .. written + buf.len()].copy_from_slice(buf);
        self.remaining -= buf.len();
    }

    #[inline(always)]
    fn remaining(&self) -> usize {
        self.remaining
    }

    #[inline(always)]
    fn done(self) -> Self::Done {
        assert_eq!(self.remaining, 0, "not all bytes written");
        self.buf
    }
}

impl<'a, B: ?Sized + AsMut<[u8]>> From<&'a mut B> for WriteCursor<'a> {
    #[inline(always)]
    fn from(buf: &'a mut B) -> WriteCursor<'a> {
        let buf = buf.as_mut();
        WriteCursor {
            remaining: buf.len(),
            buf,
        }
    }
}

impl WriteBytes for Vec<u8> {
    type Done = Self;

    #[inline(always)]
    fn write(&mut self, buf: impl AsRef<[u8]>) {
        self.extend_from_slice(buf.as_ref());
    }

    #[inline(always)]
    fn remaining(&self) -> usize {
        usize::max_value()
    }

    #[inline(always)]
    fn done(self) -> Self::Done {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_writebytes() {
        let mut v = vec![0;10];
        let mut w = WriteCursor::from(&mut v);

        w.write(&[1,2,3]);

        let mut r = w.reserve(5);

        r.write(&[4,5,6,7,8]);

        let mut w = r.done();
        w.write(&[9,10]);
        let v = w.done();

        assert_eq!(v, &[1,2,3,4,5,6,7,8,9,10]);
    }

    #[test]
    fn test_readbytes() {
        let v = vec![1u8,2,3,4,5,6,7,8,9,10];
        let mut r = &v[..];

        let mut b = [0u8;4];
        r.read_all(&mut b);
        assert_eq!(b, [1,2,3,4]);

        let mut r = r.reserve(4);
        r.read_all(&mut b);
        let mut r = r.done();

        let mut b = [0u8;2];
        r.read_all(&mut b);
        r.done();
    }
}

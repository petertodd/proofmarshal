use std::cmp;
use std::convert::{TryFrom, TryInto};
use std::fs::{File, OpenOptions};
use std::io::{self, Write, Seek, SeekFrom};
use std::marker::PhantomData;
use std::mem::{self, MaybeUninit};
use std::path::Path;
use std::slice;
use std::sync::Arc;

use memmap::Mmap;

use crate::Le;
use crate::pointee::Pointee;
use crate::offset::{OffsetMut, Offset};
use crate::pile::TryPile;
use crate::save::{self, SavePtr, SaveBlob, Save, SavePoll};

mod wordoffset;
use self::wordoffset::{Word, WordOffset};

#[derive(Debug)]
pub struct Journal<'p, H = ()> {
    marker: PhantomData<fn(&'p ()) -> &'p H>,
    mapping: Arc<Mmap>,
}

impl<H> Clone for Journal<'_, H> {
    fn clone(&self) -> Self {
        Self {
            marker: PhantomData,
            mapping: self.mapping.clone(),
        }
    }
}

impl<'p, H> Journal<'p, H> {
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let fd = OpenOptions::new()
                             .read(true)
                             .open(path)?;
        Self::open_fd(&fd)
    }

    pub fn open_fd(fd: &File) -> io::Result<Self> {
        Ok(Self {
            marker: PhantomData,
            mapping: Self::make_mapping(fd)?,
        })
    }

    fn make_mapping(fd: &File) -> io::Result<Arc<Mmap>> {
        let mapping = unsafe { Mmap::map(fd)? };

        if mapping.len() < mem::size_of::<JournalHeader<H>>() {
            todo!()
        } else {
            Ok(mapping.into())
        }
    }


    fn mapping_parts(&self) -> (&JournalHeader<H>, &[u8]) {
        let (header, rest) = self.mapping.split_at(mem::size_of::<JournalHeader<H>>());

        let header = unsafe { &*(header.as_ptr() as *const JournalHeader<H>) };
        (header, rest)
    }

    #[must_use]
    fn words(&self) -> &[Le<u64>] {
        let (_, bytes) = self.mapping_parts();
        let (prefix, words, _) = unsafe { bytes.align_to::<Word>() };
        assert_eq!(prefix.len(), 0);
        words
    }

    #[must_use]
    pub fn marks(&self) -> impl DoubleEndedIterator<Item = usize> + '_ {
        self.words().into_iter()
            .enumerate()
            .filter_map(move |(idx, word)| {
                if word.get() == !(idx as u64) {
                    Some(idx)
                } else {
                    None
                }
            })
    }

    pub fn roots<'v>(&'v self) -> impl DoubleEndedIterator<Item = TryPile<'p, 'v>> {
        self.marks().map(move |idx| {
            let slice = &self.mapping[mem::size_of::<JournalHeader<H>>() .. idx];
            unsafe { TryPile::new_unchecked(slice) }
        })
    }
}

#[derive(Debug)]
pub struct JournalMut<'p, H> {
    fd: File,
    journal: Journal<'p, H>,
}

impl<'p, H> JournalMut<'p, H> {
    pub fn create(path: impl AsRef<Path>, header: H) -> io::Result<Self> {
        let fd = OpenOptions::new()
                             .read(true)
                             .append(true)
                             .create_new(true)
                             .open(path)?;
        Self::create_from_fd(fd, header)
    }

    pub fn create_from_fd(mut fd: File, header: H) -> io::Result<Self> {
        let header = JournalHeader::new(header);
        fd.write(header.as_bytes())?;

        Self::open_fd(fd)
    }

    pub fn open(path: impl AsRef<Path>, append: bool) -> io::Result<Self> {
        let fd = OpenOptions::new()
                             .read(true)
                             .append(append)
                             .open(path)?;
        Self::open_fd(fd)
    }

    pub fn open_fd(fd: File) -> io::Result<Self> {
        let mapping = unsafe { Mmap::map(&fd)? };

        Ok(Self {
            journal: Journal::open_fd(&fd)?,
            fd,
        })
    }

    fn reload_mapping(&mut self) -> io::Result<()> {
        self.journal.mapping = Journal::<H>::make_mapping(&self.fd)?;
        Ok(())
    }

    pub fn snapshot(&self) -> Journal<'p, H> {
        self.journal.clone()
    }

    pub fn write_root<'v, 'a: 'v, T>(&'a mut self, root: &T) -> io::Result<()>
        where T: Save<OffsetMut<'p, 'v>, Offset<'static, 'static>>,
    {
        let writer = JournalWriter::new(self)?;

        let mut poll = root.init_save(&writer);
        let writer = poll.save_poll(writer)?;

        Ok(())
    }
}

#[derive(Debug)]
pub struct JournalWriter<'a, 'p: 'a, H> {
    journal: &'a mut JournalMut<'p, H>,
    buffer: Vec<u8>,
    offset: WordOffset,
}

impl<'a, 'p, H> JournalWriter<'a, 'p, H> {
    pub fn new(journal: &'a mut JournalMut<'p, H>) -> io::Result<Self> {
        let pos = journal.fd.seek(SeekFrom::End(0))?;

        let pos: usize = pos.checked_sub(mem::size_of::<JournalHeader<H>>() as u64)
                            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "journal truncated"))?
                            .try_into().expect("FIXME");
        let offset = WordOffset::align(pos);

        // FIXME: make sure the padding doesn't create a mark
        let padding = [0; mem::size_of::<Word>()];
        let padding = &padding[0 .. offset - pos];

        journal.fd.write_all(padding)?;

        Ok(Self {
            journal,
            offset,
            buffer: vec![],
        })
    }


    pub fn write_item(&mut self, len: usize) -> ItemWriter<'_> {
        ItemWriter::new(&mut self.buffer, &mut self.offset, len)
    }

    pub fn flush(&mut self) -> io::Result<()> {
        self.journal.fd.write_all(&self.buffer)?;
        self.buffer.clear();
        Ok(())
    }

    pub fn commit(&mut self) -> io::Result<WordOffset> {
        self.flush()?;

        let idx_words = self.offset.get() / mem::size_of::<Word>();
        let mark = (!idx_words).to_le_bytes();
        self.journal.fd.write(&mark)?;
        self.offset += WordOffset::WORD;
        self.journal.reload_mapping()?;

        // FIXME: verify mapping is correct size/file hasn't been truncated
        Ok(self.offset - WordOffset::WORD)
    }
}

impl<'p, 'v, H> SavePtr for JournalWriter<'v, 'p, H> {
    type Source = OffsetMut<'p, 'v>;
    type Target = Offset<'static, 'static>;
    type Error = io::Error;

    unsafe fn check_dirty<'a, T: ?Sized>(&self, ptr: &'a Self::Source, metadata: T::Metadata) -> Result<Self::Target, &'a T>
        where T: Pointee
    {
        todo!()
    }

    fn try_save_ptr(self, saver: &impl SaveBlob) -> Result<(Self, Self::Target), Self::Error> {
        todo!()
    }
}

#[derive(Debug)]
pub struct ItemAllocator<'a, 'v, 'p, H>(&'a mut JournalWriter<'v, 'p, H>);

impl<'a, 'p, 'v, H> save::AllocBlob for ItemAllocator<'a, 'v, 'p, H> {
    type WriteBlob = ItemWriter<'a>;
    type Error = io::Error;
    type Done = WordOffset;

    fn alloc_blob(mut self, size: usize) -> Result<Self::WriteBlob, Self::Error> {
        Ok(self.0.write_item(size))
    }
}

impl<'a> save::WriteBlob for ItemWriter<'a> {
    type Done = WordOffset;
    type Error = io::Error;

    fn write_bytes(mut self, buf: &[u8]) -> Result<Self, Self::Error> {
        ItemWriter::write_bytes(&mut self, buf);
        Ok(self)
    }

    fn done(self) -> Result<Self::Done, Self::Error> {
        Ok(self.finish())
    }
}


#[derive(Debug)]
pub struct ItemWriter<'a> {
    buffer: &'a mut Vec<u8>,
    offset: &'a mut WordOffset,
    capacity: usize,
    idx: usize,
}

impl<'a> ItemWriter<'a> {
    #[inline]
    fn new(buffer: &'a mut Vec<u8>, offset: &'a mut WordOffset, len: usize) -> Self {
        assert!(len <= isize::MAX as usize, "overflow");

        let capacity = buffer.len() + len;
        buffer.reserve(len + mem::size_of::<Word>());
        Self {
            capacity: buffer.len() + len,
            idx: buffer.len(),
            buffer,
            offset,
        }
    }

    #[inline]
    pub fn write_bytes(&mut self, src: &[u8]) {
        let remaining = self.capacity - self.idx;
        assert!(src.len() <= remaining, "overflow");

        unsafe {
            let dst = self.buffer.as_mut_ptr()
                          .offset(self.idx as isize);

            dst.copy_from(src.as_ptr(), src.len());
            self.idx += src.len();
        }
    }

    pub fn finish(self) -> WordOffset {
        let start = self.buffer.len();
        let end = self.idx;
        assert_eq!(self.idx, self.capacity, "not all bytes written");

        // SAFETY: all bytes up to capacity have been initialized
        unsafe { self.buffer.set_len(self.capacity) }

        // Write zeros to align the end
        for _ in 0 .. WordOffset::align_padding(self.buffer.len()) {
            self.buffer.push(0);
        }

        let written_bytes = &self.buffer[start..];
        let written_bytes_len = written_bytes.len();

        let (prefix, written_words, suffix) = unsafe { written_bytes.align_to::<Word>() };
        assert_eq!(prefix.len(), 0);
        assert_eq!(suffix.len(), 0);

        let padding_len_words = calc_conflicts(self.offset.get() / mem::size_of::<Word>(), written_words);
        let padding_len_bytes = padding_len_words * mem::size_of::<Word>();
        if padding_len_bytes > 0 {
            self.buffer.resize(self.buffer.len() + padding_len_bytes, 0);
            self.buffer.copy_within(self.offset.get() .. written_bytes_len, self.offset.get() + padding_len_bytes);

            *self.offset += WordOffset::try_from(padding_len_bytes).unwrap();
        }

        let r = *self.offset;
        *self.offset += WordOffset::try_from(written_bytes_len).unwrap();
        r
    }
}


fn calc_conflicts(offset: usize, words: &[Le<u64>]) -> usize {
    if words.len() <= 32 {
        let mut conflicts = [false; 32];
        calc_conflicts_impl(offset, words, &mut conflicts[0 .. words.len()])
    } else {
        let mut conflicts = vec![false; words.len()];
        calc_conflicts_impl(offset, words, &mut conflicts)
    }
}

fn calc_conflicts_impl(offset: usize, words: &[Le<u64>], conflicts: &mut [bool]) -> usize {
    assert_eq!(words.len(), conflicts.len());
    conflicts.iter_mut().for_each(|b| *b = false);

    for (i, word) in words.iter().enumerate() {
        let mark_idx = (offset + i) as u64;
        let potential_mark = !word.get();
        if let Some(conflict) = potential_mark.checked_sub(mark_idx)
                                              .and_then(|idx| conflicts.get_mut(idx as usize))
        {
            *conflict = true;
        }
    }

    for (i, conflict) in conflicts.iter().enumerate() {
        if !conflict {
            return i;
        }
    }
    conflicts.len()
}

#[derive(Default)]
struct JournalHeader<H = ()> {
    magic: [u8; 16],
    header: H,
}

impl<H> JournalHeader<H> {
    pub fn new(header: H) -> Self {
        Self {
            magic: [0; 16],
            header,
        }
    }
    pub fn as_bytes(&self) -> &[u8] {
        assert_eq!(mem::size_of::<H>(), 0);
        unsafe {
            slice::from_raw_parts(self as *const Self as *const u8, mem::size_of::<Self>())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::tempfile;

    #[test]
    fn test_calc_conflicts() {
        #[track_caller]
        fn test(offset: usize, words: &[u64], expected_conflicts: &[bool], expected_min_move: usize) {
            let words: Vec<Le<u64>> = words.iter().map(|word| Le::new(!word)).collect();
            let mut conflicts = vec![false; words.len()];

            let actual_min_move = calc_conflicts_impl(offset, &words, &mut conflicts);
            assert_eq!(conflicts, expected_conflicts,
                       "conflicts didn't match");
            assert_eq!(actual_min_move, expected_min_move,
                       "actual min move != expected min move");

            assert_eq!(calc_conflicts(offset + actual_min_move, &words), 0,
                       "actual != expected");
        }

        test(0, &[], &[], 0);
        test(0, &[0], &[true], 1);
        test(0, &[1], &[false], 0);
        test(0, &[1,1], &[true, true], 2);
        test(0, &[1,0], &[false, true], 0);

        test(0, &[0,2,4,6,8], &[true;5], 5);

        test(0, &[0,40,6,6,8],
                &[true, false, false, true, true], 1);
    }

    #[test]
    fn test_conflicts() {
        let mut dst = vec![];
        let mut offset = WordOffset::try_from(0).unwrap();

        // A zero-length entry
        let writer = ItemWriter::new(&mut dst, &mut offset, 0);
        assert_eq!(writer.finish(), 0);

        // Non-conflicting
        let mut writer = ItemWriter::new(&mut dst, &mut offset, 16);
        writer.write_bytes(&[0xf0,0xff,0xff,0xff,0xff,0xff,0xff,0xff,
                             0xf0,0xff,0xff,0xff,0xff,0xff,0xff,0xff]);
        dbg!(&writer);
        assert_eq!(writer.finish(), 0);

        // Conflicting on first word
        let mut dst = vec![];
        let mut offset = WordOffset::try_from(0).unwrap();
        let mut writer = ItemWriter::new(&mut dst, &mut offset, 16);
        writer.write_bytes(&[0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,
                             0xf0,0xff,0xff,0xff,0xff,0xff,0xff,0xff]);
        assert_eq!(writer.finish(), 8);

        // Conflicting on both words
        let mut dst = vec![];
        let mut offset = WordOffset::try_from(0).unwrap();
        let mut writer = ItemWriter::new(&mut dst, &mut offset, 16);
        writer.write_bytes(&[0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,
                             0xfd,0xff,0xff,0xff,0xff,0xff,0xff,0xff]);
        assert_eq!(writer.finish(), 16);
    }

    #[test]
    fn journal_create() -> io::Result<()> {
        let mut journal = JournalMut::create_from_fd(tempfile()?, ())?;

        let snapshot = journal.snapshot();
        let marks = snapshot.marks().collect::<Vec<_>>();
        assert_eq!(marks.len(), 0);

        let mut writer = JournalWriter::new(&mut journal)?;

        let entry_bytes = &[0x1b; 10];
        let mut entry = writer.write_item(entry_bytes.len());
        entry.write_bytes(entry_bytes);
        let entry_offset = entry.finish();

        let tip_offset = writer.commit()?;
        assert_eq!(entry_offset + WordOffset::align(entry_bytes.len()), tip_offset);

        // snapshot doesn't change
        let marks = snapshot.marks().collect::<Vec<_>>();
        assert_eq!(marks.len(), 0);

        // but the reloaded one does
        let snapshot = dbg!(journal.snapshot());

        let marks = snapshot.marks().collect::<Vec<_>>();
        assert_eq!(marks, &[2]);

        Ok(())
    }

    #[test]
    fn journal_roots() -> io::Result<()> {
        let mut journal = JournalMut::create_from_fd(tempfile()?, ())?;

        let snapshot = journal.snapshot();
        assert!(snapshot.roots().last().is_none());

        Ok(())
    }
}

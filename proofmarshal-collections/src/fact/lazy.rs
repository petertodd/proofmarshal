#![allow(dead_code)]

use std::cell::UnsafeCell;
use std::fmt;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicU8, Ordering, spin_loop_hint};
use std::ptr;


pub struct Lazy<T> {
    state: AtomicU8,
    cell: UnsafeCell<MaybeUninit<T>>,
}

static_assertions::assert_eq_align!(Lazy<u8>, u8);
static_assertions::assert_eq_size!(Lazy<u8>, Option<u8>);

unsafe impl<T: Sync> Sync for Lazy<T> {}

#[repr(u8)]
#[derive(Debug)]
enum State {
    None = 0,
    Some = 1,
    Pending = 2,
}

fn check_state(n: u8) -> State {
    match n {
        0 => State::None,
        1 => State::Some,
        2 => State::Pending,
        x => invalid_state(x),
    }
}

fn invalid_state(x: u8) -> ! {
    debug_assert!(false, "invalid Lazy<T> state {}", x);
    unsafe { core::hint::unreachable_unchecked() }
}

fn invalid_pending() -> ! {
    debug_assert!(false, "Lazy<T> in pending write state, yet owned");
    unsafe { core::hint::unreachable_unchecked() }
}

impl<T> Lazy<T> {
    pub const fn none() -> Self {
        Self {
            state: AtomicU8::new(State::None as u8),
            cell: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }

    pub const fn some(value: T) -> Self {
        Self {
            state: AtomicU8::new(State::Some as u8),
            cell: UnsafeCell::new(MaybeUninit::new(value)),
        }
    }

    pub fn get(&self) -> Option<&T> {
        loop {
            match check_state(self.state.load(Ordering::Acquire)) {
                State::None => break None,
                State::Some => break unsafe {
                    let uninit: &MaybeUninit<T> = &*self.cell.get();
                    Some(&*uninit.as_ptr())
                },
                State::Pending => {
                    spin_loop_hint();
                    continue
                },
            }
        }
    }

    pub fn try_set(&self, value: T) -> Result<(), T> {
        match check_state(self.state.compare_and_swap(
                State::None as u8, State::Pending as u8,
                Ordering::Release))
        {
            // The value has already been set, or something is in the process of setting it.
            State::Some | State::Pending => Err(value),

            State::None => {
                // Safe because we've set the state to PENDING, preventing all other threads from
                // accessing the value
                unsafe {
                    (self.cell.get() as *mut T).write(value)
                };

                self.state.store(State::Some as u8, Ordering::Release);
                Ok(())
            },
        }
    }

    pub fn get_mut(&mut self) -> Option<&mut T> {
        match check_state(*self.state.get_mut()) {
            State::None => None,
            State::Some => {
                // Safe because our &mut ownership statically guarantees we're the only accessor.
                let u: &mut MaybeUninit<T> = unsafe { self.cell.get().as_mut().unwrap() };

                // Safe because we're guaranteed to be initialized
                Some(unsafe { &mut *u.as_mut_ptr() })
            }
            State::Pending => invalid_pending(),
        }
    }

    pub fn set(&mut self, value: T) {
        // Verify that we're not in a pending state.
        let _ = self.get_mut();

        self.cell = MaybeUninit::new(value).into();
        self.state = AtomicU8::new(State::Some as u8);
    }

    pub fn take(&mut self) -> Option<T> {
        match self.get_mut() {
            Some(value) => {
                let r = unsafe { ptr::read(value) };
                self.state = AtomicU8::new(State::None as u8);
                Some(r)
            }
            None => None,
        }
    }
}


impl<T: fmt::Debug> fmt::Debug for Lazy<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.get() {
            Some(r) => f.debug_tuple("Some")
                        .field(r)
                        .finish(),
            None => f.debug_tuple("None")
                     .finish(),
        }
    }
}

impl<T> From<Option<T>> for Lazy<T> {
    fn from(opt: Option<T>) -> Self {
        match opt {
            Some(value) => Lazy::some(value),
            None => Lazy::none(),
        }
    }
}

impl<T: Clone> Clone for Lazy<T> {
    fn clone(&self) -> Self {
        self.get().cloned().into()
    }
}

impl<T> Drop for Lazy<T> {
    fn drop(&mut self) {
        self.take();
    }
}

pub fn test_lazy_get(x: &Lazy<u8>, y: &Lazy<u8>) -> Option<u8> {
    match (x.get(), y.get()) {
        (Some(x), Some(y)) => Some(x + y),
        _ => None,
    }
}

pub fn test_try_set(lazy: &Lazy<u8>, value: u8) -> Result<(), u8> {
    lazy.try_set(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_set() {
        let lazy: Lazy<u8> = dbg!(Lazy::none());
        assert_eq!(lazy.get(), None);

        lazy.try_set(42).unwrap();
        assert_eq!(lazy.get(), Some(&42));

        // Second set fails
        assert_eq!(lazy.try_set(42).unwrap_err(),
                   42);

        assert_eq!(lazy.try_set(123).unwrap_err(),
                   123);

        // Value unchanged
        assert_eq!(lazy.get(), Some(&42));
    }
}

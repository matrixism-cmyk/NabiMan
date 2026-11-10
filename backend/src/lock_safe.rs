//! Locks that outlive a panic.
//!
//! A `std::sync::Mutex` is poisoned when a thread panics while holding it,
//! and `lock().unwrap()` then panics in every later caller: one bad request
//! on the sign-in, audit or session path would fail every request after it
//! until a restart. The values kept behind these locks are whole between
//! statements (maps, lists, caches, settings), so a poisoned lock is simply
//! taken up again.
//!
//! The one store that must not carry on as if nothing happened is the audit
//! chain: a panic in the middle of an append could leave the file and the
//! chain's last hash out of step. `audit::append_entry` checks for poisoning
//! itself and stops persisting (reported as unavailable) instead.

use std::sync::{Mutex, MutexGuard, PoisonError};

pub trait LockExt<T: ?Sized> {
    /// Lock, taking the value up again if an earlier holder panicked.
    fn lock_safe(&self) -> MutexGuard<'_, T>;
}

impl<T: ?Sized> LockExt<T> for Mutex<T> {
    fn lock_safe(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::LockExt;
    use std::sync::{Arc, Mutex};

    fn poison<T: Send + 'static>(m: &Arc<Mutex<T>>) {
        let m = m.clone();
        let _ = std::thread::spawn(move || {
            let _guard = m.lock().unwrap();
            panic!("a request handler panicked while holding the lock");
        })
        .join();
    }

    #[test]
    fn a_poisoned_lock_still_serves_its_value() {
        let m = Arc::new(Mutex::new(vec![1, 2]));
        poison(&m);
        assert!(m.is_poisoned());
        m.lock_safe().push(3);
        assert_eq!(*m.lock_safe(), vec![1, 2, 3]);
    }
}

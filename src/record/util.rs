use std::sync::{Condvar, Mutex};

/// Combination of a [`Mutex`] and a corresponding [`Condvar`].
/// Works like a spinlock, but is efficient.
///
/// As long as `*continue_work == true`, the work should be
/// continued. When it is set to false, the work loop should
/// be exited.
#[derive(Debug, Default)]
pub struct CondVarSpinlock {
    continue_work_mutex: Mutex<bool>,
    condvar: Condvar,
}

impl CondVarSpinlock {
    const WORK_DO: bool = true;
    const WORK_STOPPED: bool = false;

    pub fn new() -> Self {
        Self {
            continue_work_mutex: Mutex::new(Self::WORK_DO),
            condvar: Condvar::new(),
        }
    }

    pub fn is_stopped(&self) -> bool {
        *self.continue_work_mutex.lock().unwrap() == Self::WORK_STOPPED
    }

    pub fn block_until_stopped(&self) {
        let _lock = self
            .condvar
            .wait_while(self.continue_work_mutex.lock().unwrap(), |continue_work| {
                *continue_work
            })
            .unwrap();
        /*let mut guard = self.continue_work_mutex.lock().unwrap();
        while *guard == Self::WORK_DO {
            guard = self.condvar.wait(guard).unwrap();
        }*/
    }

    pub fn stop_work(&self) {
        let mut lock = self.continue_work_mutex.lock().unwrap();
        *lock = Self::WORK_STOPPED;
        self.condvar.notify_one();
    }
}

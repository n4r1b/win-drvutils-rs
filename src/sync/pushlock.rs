use crate::inner_getters_value;
use windows_sys::Wdk::System::SystemServices::{
    ExAcquirePushLockExclusiveEx, ExAcquirePushLockSharedEx, ExInitializePushLock,
    ExReleasePushLockExclusiveEx, ExReleasePushLockSharedEx, EX_DEFAULT_PUSH_LOCK_FLAGS,
};

enum PushLockAccess {
    None,
    Exclusive,
    Shared,
}

pub struct WduPushLock {
    access: PushLockAccess,
    pushlock: usize,
}

impl Default for WduPushLock {
    fn default() -> Self {
        Self::new()
    }
}

inner_getters_value!(WduPushLock, pushlock, usize);

impl WduPushLock {
    #[cfg(feature = "const_new")]
    pub const fn const_new() -> Self {
        Self {
            access: PushLockAccess::None,
            pushlock: 0,
        }
    }

    pub fn new() -> Self {
        WduPushLock {
            access: PushLockAccess::None,
            pushlock: usize::default(),
        }
    }

    pub fn init(&mut self) {
        unsafe {
            ExInitializePushLock(self.as_mut_ptr());
        }
    }

    pub fn acquired_shared(&mut self) {
        unsafe {
            ExAcquirePushLockSharedEx(self.as_mut_ptr(), EX_DEFAULT_PUSH_LOCK_FLAGS);
        }
        self.access = PushLockAccess::Shared;
    }

    pub fn acquire_exclusive(&mut self) {
        unsafe {
            ExAcquirePushLockExclusiveEx(self.as_mut_ptr(), EX_DEFAULT_PUSH_LOCK_FLAGS);
        }
        self.access = PushLockAccess::Exclusive;
    }

    pub fn release(&mut self) {
        unsafe {
            match self.access {
                PushLockAccess::Shared => {
                    ExReleasePushLockSharedEx(self.as_mut_ptr(), EX_DEFAULT_PUSH_LOCK_FLAGS)
                }
                PushLockAccess::Exclusive => {
                    ExReleasePushLockExclusiveEx(self.as_mut_ptr(), EX_DEFAULT_PUSH_LOCK_FLAGS)
                }
                PushLockAccess::None => (),
            }
        }
    }
}

#[cfg(feature = "lock_api")]
pub mod lock_api {
    use super::WduPushLock;
    use crate::sync::{enter_critical_region, leave_critical_region};

    pub type WduPushLockRw<T> = lock_api::RwLock<WduPushLock, T>;

    impl WduPushLock {
        pub fn init_lock(&self) {
            let ptr = &self as *const _ as *mut Self;
            unsafe {
                (*ptr).init();
            }
        }

        pub fn lock_shared(&self) {
            let ptr = &self as *const _ as *mut Self;
            unsafe {
                (*ptr).acquired_shared();
            }
        }

        pub fn lock_exclusive(&self) {
            let ptr = &self as *const _ as *mut Self;
            unsafe {
                (*ptr).acquire_exclusive();
            }
        }

        pub unsafe fn unlock(&self) {
            let ptr = &self as *const _ as *mut Self;
            (*ptr).release();
        }
    }

    #[allow(dead_code)]
    unsafe impl lock_api::RawRwLock for WduPushLock {
        type GuardMarker = lock_api::GuardSend;

        const INIT: Self = Self::const_new();

        fn lock_shared(&self) {
            enter_critical_region();
            self.lock_shared();
        }

        fn try_lock_shared(&self) -> bool {
            unimplemented!()
        }

        unsafe fn unlock_shared(&self) {
            self.unlock();
            leave_critical_region();
        }

        fn lock_exclusive(&self) {
            enter_critical_region();
            self.lock_exclusive();
        }

        fn try_lock_exclusive(&self) -> bool {
            unimplemented!()
        }

        unsafe fn unlock_exclusive(&self) {
            self.unlock();
            leave_critical_region();
        }
    }
}

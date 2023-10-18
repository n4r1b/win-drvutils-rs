use crate::{current_irql, inner_getters_value};
use core::borrow::{Borrow, BorrowMut};
use windows_sys::Wdk::System::SystemServices::{
    KeAcquireInStackQueuedSpinLock, KeAcquireSpinLockForDpc, KeGetCurrentIrql,
    KeInitializeSpinLock, KeReleaseInStackQueuedSpinLock, KeReleaseSpinLockForDpc, DISPATCH_LEVEL,
    KLOCK_QUEUE_HANDLE, PASSIVE_LEVEL,
};

// Not available in windows-sys v0.52
extern "system" {
    fn KeAcquireSpinLockRaiseToDpc(spinlock: *mut usize) -> u8;
    fn KeAcquireSpinLockAtDpcLevel(spinlock: *mut usize);
    fn KeReleaseSpinLock(spinlock: *mut usize, new_irql: u8);
    fn KeReleaseSpinLockFromDpcLevel(spinlock: *mut usize);
}

struct LockHandle(KLOCK_QUEUE_HANDLE);

impl Default for LockHandle {
    fn default() -> Self {
        LockHandle(unsafe { core::mem::zeroed() })
    }
}

impl LockHandle {
    fn inner(&self) -> *const KLOCK_QUEUE_HANDLE {
        self.0.borrow()
    }

    fn inner_mut(&mut self) -> *mut KLOCK_QUEUE_HANDLE {
        self.0.borrow_mut()
    }
}

pub struct WduSpinLock {
    old_irql: u8,
    spinlock: usize,
    lockhandle: Option<LockHandle>,
}

// TODO
// pub struct WduSpinlockEx {
//     irql: u8,
//     spinlock: u32,
// }

impl Default for WduSpinLock {
    fn default() -> Self {
        Self::new()
    }
}

inner_getters_value!(WduSpinLock, spinlock, usize);

impl WduSpinLock {
    #[cfg(feature = "lock_api")]
    pub const fn const_new() -> Self {
        Self {
            old_irql: 0,
            spinlock: 0usize,
            lockhandle: None,
        }
    }

    pub fn new() -> Self {
        Self {
            old_irql: 0,
            spinlock: usize::default(),
            lockhandle: None,
        }
    }

    pub fn init(&mut self) {
        unsafe { KeInitializeSpinLock(self.as_mut_ptr()) }
    }

    #[inline(always)]
    pub fn acquire(&mut self) {
        debug_assert!(current_irql() <= DISPATCH_LEVEL as u8);

        self.old_irql = unsafe { KeAcquireSpinLockRaiseToDpc(self.as_mut_ptr()) };
    }

    #[inline(always)]
    pub fn release(&mut self) {
        debug_assert!(current_irql() == DISPATCH_LEVEL as u8);

        unsafe {
            KeReleaseSpinLock(self.as_mut_ptr(), self.old_irql);
        }
    }

    #[inline(always)]
    pub fn acquire_at_dpc(&mut self) {
        debug_assert!(current_irql() >= DISPATCH_LEVEL as u8);

        unsafe { KeAcquireSpinLockAtDpcLevel(self.as_mut_ptr()) };
    }

    #[inline(always)]
    pub fn release_from_dpc(&mut self) {
        debug_assert!(current_irql() >= DISPATCH_LEVEL as u8);

        unsafe {
            KeReleaseSpinLockFromDpcLevel(self.as_mut_ptr());
        }
    }

    #[inline(always)]
    pub fn acquire_in_stack(&mut self) {
        debug_assert!(current_irql() <= DISPATCH_LEVEL as u8);
        debug_assert!(self.lockhandle.is_none());

        self.lockhandle = Some(LockHandle::default());
        unsafe {
            KeAcquireInStackQueuedSpinLock(
                self.as_mut_ptr(),
                self.lockhandle.as_mut().unwrap().inner_mut(),
            )
        }
    }

    #[inline(always)]
    pub fn release_in_stack(&mut self) {
        debug_assert!(self.lockhandle.is_some());

        unsafe { KeReleaseInStackQueuedSpinLock(self.lockhandle.as_ref().unwrap().inner()) }
        self.lockhandle = None;
    }

    #[inline(always)]
    pub fn acquire_for_dpc(&mut self) {
        debug_assert!(current_irql() <= DISPATCH_LEVEL as u8);

        self.old_irql = unsafe { KeAcquireSpinLockForDpc(self.as_mut_ptr()) };
    }

    #[inline(always)]
    pub fn release_for_dpc(&mut self) {
        debug_assert!(current_irql() == DISPATCH_LEVEL as u8);

        unsafe {
            KeReleaseSpinLockForDpc(self.as_mut_ptr(), self.old_irql);
        }
    }
}

// TODO: Create macro for internal types
#[cfg(feature = "lock_api")]
pub mod lock_api {
    use super::WduSpinLock;
    use core::ops::{Deref, DerefMut};

    pub struct SpinLock(WduSpinLock);
    pub struct StackSpinLock(WduSpinLock);
    pub struct DpcSpinLock(WduSpinLock);

    pub type WduSpinLockMtx<T> = lock_api::Mutex<SpinLock, T>;
    pub type WduStackSpinLockMtx<T> = lock_api::Mutex<StackSpinLock, T>;
    pub type WduDpcSpinLockMtx<T> = lock_api::Mutex<DpcSpinLock, T>;

    impl Deref for SpinLock {
        type Target = WduSpinLock;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl DerefMut for SpinLock {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }

    impl Deref for StackSpinLock {
        type Target = WduSpinLock;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl DerefMut for StackSpinLock {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }

    impl Deref for DpcSpinLock {
        type Target = WduSpinLock;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl DerefMut for DpcSpinLock {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }

    impl SpinLock {
        fn as_mut_ptr(&self) -> *mut WduSpinLock {
            &self.0 as *const _ as *mut _
        }

        pub fn init_lock(&self) {
            let ptr = self.as_mut_ptr();
            unsafe {
                (*ptr).init();
            }
        }

        pub fn lock(&self) {
            let ptr = self.as_mut_ptr();
            unsafe {
                (*ptr).acquire();
            }
        }

        pub fn unlock(&self) {
            let ptr = self.as_mut_ptr();
            unsafe {
                (*ptr).release();
            }
        }
    }

    impl StackSpinLock {
        fn as_mut_ptr(&self) -> *mut WduSpinLock {
            &self.0 as *const _ as *mut _
        }

        pub fn init_lock(&self) {
            let ptr = self.as_mut_ptr();
            unsafe {
                (*ptr).init();
            }
        }

        pub fn lock_stack(&self) {
            let ptr = self.as_mut_ptr();
            unsafe {
                (*ptr).acquire_in_stack();
            }
        }

        pub fn unlock_stack(&self) {
            let ptr = self.as_mut_ptr();
            unsafe {
                (*ptr).release_in_stack();
            }
        }
    }

    impl DpcSpinLock {
        fn as_mut_ptr(&self) -> *mut WduSpinLock {
            &self.0 as *const _ as *mut _
        }

        pub fn init_lock(&self) {
            let ptr = self.as_mut_ptr();
            unsafe {
                (*ptr).init();
            }
        }

        pub fn lock_at_dpc(&self) {
            let ptr = self.as_mut_ptr();
            unsafe {
                (*ptr).acquire_at_dpc();
            }
        }

        pub fn unlock_at_dpc(&self) {
            let ptr = self.as_mut_ptr();
            unsafe {
                (*ptr).release_from_dpc();
            }
        }
    }

    unsafe impl lock_api::RawMutex for SpinLock {
        type GuardMarker = lock_api::GuardNoSend;

        const INIT: Self = Self(WduSpinLock::const_new());

        fn lock(&self) {
            self.lock();
        }

        fn try_lock(&self) -> bool {
            unimplemented!()
        }

        unsafe fn unlock(&self) {
            self.unlock();
        }
    }

    unsafe impl lock_api::RawMutex for StackSpinLock {
        type GuardMarker = lock_api::GuardNoSend;

        const INIT: Self = Self(WduSpinLock::const_new());

        fn lock(&self) {
            self.lock_stack();
        }

        fn try_lock(&self) -> bool {
            unimplemented!()
        }

        unsafe fn unlock(&self) {
            self.unlock_stack();
        }
    }

    unsafe impl lock_api::RawMutex for DpcSpinLock {
        type GuardMarker = lock_api::GuardNoSend;

        const INIT: Self = Self(WduSpinLock::const_new());

        fn lock(&self) {
            self.lock_at_dpc();
        }

        fn try_lock(&self) -> bool {
            unimplemented!()
        }

        unsafe fn unlock(&self) {
            self.unlock_at_dpc();
        }
    }
}

use crate::inner_getters_value;
use core::borrow::BorrowMut;
use windows_sys::{
    Wdk::{
        Foundation::{FAST_MUTEX, KMUTANT},
        System::SystemServices::{
            ExAcquireFastMutex, ExAcquireFastMutexUnsafe, ExReleaseFastMutex,
            ExReleaseFastMutexUnsafe, ExTryToAcquireFastMutex, KeAcquireGuardedMutex,
            KeAcquireGuardedMutexUnsafe, KeInitializeEvent, KeInitializeGuardedMutex,
            KeInitializeMutex, KeReadStateMutex, KeReleaseGuardedMutex,
            KeReleaseGuardedMutexUnsafe, KeReleaseMutex, KeTryToAcquireGuardedMutex, FM_LOCK_BIT,
        },
    },
    Win32::System::Kernel::SynchronizationEvent,
};

#[cfg(feature = "const_new")]
use const_zero::const_zero;

pub struct WduMutant {
    mutex: KMUTANT,
}

impl Default for WduMutant {
    fn default() -> Self {
        Self::new()
    }
}

pub struct WduFastMutex {
    mutex: FAST_MUTEX,
}

impl Default for WduFastMutex {
    fn default() -> Self {
        Self::new()
    }
}

pub struct WduGuardedMutex {
    mutex: FAST_MUTEX,
}

impl Default for WduGuardedMutex {
    fn default() -> Self {
        Self::new()
    }
}

inner_getters_value!(WduMutant, mutex, KMUTANT);
inner_getters_value!(WduFastMutex, mutex, FAST_MUTEX);
inner_getters_value!(WduGuardedMutex, mutex, FAST_MUTEX);

impl WduMutant {
    #[cfg(feature = "const_new")]
    pub const fn const_new() -> Self {
        unsafe { const_zero!(WduMutant) }
    }

    pub fn new() -> Self {
        WduMutant {
            mutex: unsafe { core::mem::zeroed() },
        }
    }

    pub fn init(&mut self) {
        unsafe { KeInitializeMutex(self.as_mut_ptr(), 0) }
    }

    pub fn release(&mut self, wait: bool) {
        unsafe {
            KeReleaseMutex(self.as_mut_ptr(), u8::from(wait));
        }
    }

    pub fn read_state(&mut self) -> i32 {
        unsafe { KeReadStateMutex(self.as_mut_ptr()) }
    }
}

impl WduFastMutex {
    #[cfg(feature = "const_new")]
    pub const fn const_new() -> Self {
        unsafe { const_zero!(WduFastMutex) }
    }

    pub fn new() -> Self {
        WduFastMutex {
            mutex: unsafe { core::mem::zeroed() },
        }
    }

    /*
    FORCEINLINE
    VOID
    ExInitializeFastMutex (
        _Out_ PFAST_MUTEX FastMutex
        )
    {
        WriteRaw(&FastMutex->Count, FM_LOCK_BIT);
        FastMutex->Owner = NULL;
        FastMutex->Contention = 0;
        KeInitializeEvent(&FastMutex->Event, SynchronizationEvent, FALSE);
        return;
    }
    */
    #[inline(always)]
    pub fn init(&mut self) {
        self.mutex.Count = FM_LOCK_BIT as i32;
        unsafe {
            KeInitializeEvent(
                self.mutex.Event.borrow_mut(),
                SynchronizationEvent,
                u8::from(false),
            );
        }
    }

    pub fn acquire(&mut self) {
        unsafe {
            ExAcquireFastMutex(self.as_mut_ptr());
        }
    }

    pub fn acquire_unsafe(&mut self) {
        unsafe {
            ExAcquireFastMutexUnsafe(self.as_mut_ptr());
        }
    }

    pub fn try_acquire(&mut self) -> bool {
        unsafe { ExTryToAcquireFastMutex(self.as_mut_ptr()) == u8::from(true) }
    }

    pub fn release(&mut self) {
        unsafe {
            ExReleaseFastMutex(self.as_mut_ptr());
        }
    }

    pub fn release_unsafe(&mut self) {
        unsafe {
            ExReleaseFastMutexUnsafe(self.as_mut_ptr());
        }
    }
}

impl WduGuardedMutex {
    #[cfg(feature = "const_new")]
    pub const fn const_new() -> Self {
        unsafe { const_zero!(WduGuardedMutex) }
    }

    pub fn new() -> Self {
        WduGuardedMutex {
            mutex: unsafe { core::mem::zeroed() },
        }
    }

    pub fn init(&mut self) {
        unsafe { KeInitializeGuardedMutex(self.as_mut_ptr()) }
    }

    pub fn acquire(&mut self) {
        unsafe {
            KeAcquireGuardedMutex(self.as_mut_ptr());
        }
    }

    pub fn acquire_unsafe(&mut self) {
        unsafe {
            KeAcquireGuardedMutexUnsafe(self.as_mut_ptr());
        }
    }

    pub fn try_acquire(&mut self) -> bool {
        unsafe { KeTryToAcquireGuardedMutex(self.as_mut_ptr()) == u8::from(true) }
    }

    pub fn release(&mut self) {
        unsafe {
            KeReleaseGuardedMutex(self.as_mut_ptr());
        }
    }

    pub fn release_unsafe(&mut self) {
        unsafe {
            KeReleaseGuardedMutexUnsafe(self.as_mut_ptr());
        }
    }
}

#[cfg(feature = "lock_api")]
pub mod lock_api {
    use super::{WduFastMutex, WduGuardedMutex};
    use core::borrow::BorrowMut;

    pub type WduFastMtx<T> = lock_api::Mutex<WduFastMutex, T>;
    pub type WduGuardedMtx<T> = lock_api::Mutex<WduGuardedMutex, T>;

    impl WduFastMutex {
        pub fn init_lock(&self) {
            let ptr = &self as *const _ as *mut Self;
            unsafe {
                (*ptr).init();
            }
        }

        pub fn lock(&self) {
            let ptr = &self as *const _ as *mut Self;
            unsafe {
                (*ptr).acquire();
            }
        }

        pub fn try_lock(&self) -> bool {
            let ptr = &self as *const _ as *mut Self;
            unsafe { (*ptr).try_acquire() }
        }

        pub fn unlock(&self) {
            let ptr = &self as *const _ as *mut Self;
            unsafe { (*ptr).release() }
        }
    }

    impl WduGuardedMutex {
        pub fn init_lock(&self) {
            let ptr = &self as *const _ as *mut Self;
            unsafe { (*ptr).init() }
        }

        pub fn lock(&self) {
            let ptr = &self as *const _ as *mut Self;
            unsafe { (*ptr).acquire() }
        }

        pub fn try_lock(&self) -> bool {
            let ptr = &self as *const _ as *mut Self;
            unsafe { (*ptr).try_acquire() }
        }

        pub fn unlock(&self) {
            let ptr = &self as *const _ as *mut Self;
            unsafe { (*ptr).release() }
        }
    }

    unsafe impl lock_api::RawMutex for WduFastMutex {
        // TODO: Double Check!
        type GuardMarker = lock_api::GuardSend;

        const INIT: Self = Self::const_new();

        fn lock(&self) {
            self.lock();
        }

        fn try_lock(&self) -> bool {
            self.try_lock()
        }

        unsafe fn unlock(&self) {
            self.unlock();
        }
    }

    unsafe impl lock_api::RawMutex for WduGuardedMutex {
        // TODO: Double Check!
        type GuardMarker = lock_api::GuardSend;

        const INIT: Self = Self::const_new();

        fn lock(&self) {
            self.lock();
        }

        fn try_lock(&self) -> bool {
            self.try_lock()
        }

        unsafe fn unlock(&self) {
            self.unlock();
        }
    }
}

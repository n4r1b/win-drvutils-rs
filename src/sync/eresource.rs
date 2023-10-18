use crate::inner_getters_value;
use windows_sys::Wdk::{
    Foundation::ERESOURCE,
    System::SystemServices::{
        ExAcquireResourceExclusiveLite, ExAcquireResourceSharedLite,
        ExConvertExclusiveToSharedLite, ExDeleteResourceLite, ExInitializeResourceLite,
        ExReleaseResourceLite,
    },
};

#[cfg(feature = "const_new")]
use const_zero::const_zero;

pub struct WduEResource {
    resource: ERESOURCE,
}

impl Default for WduEResource {
    fn default() -> Self {
        let mut resource = Self {
            resource: unsafe { core::mem::zeroed() },
        };

        resource.init();
        resource
    }
}

impl Drop for WduEResource {
    fn drop(&mut self) {
        self.delete()
    }
}

inner_getters_value!(WduEResource, resource, ERESOURCE);

impl WduEResource {
    #[cfg(feature = "const_new")]
    pub const fn const_new() -> Self {
        unsafe { const_zero!(WduEResource) }
    }

    fn init(&mut self) {
        unsafe {
            // ExInitializeResourceLite returns STATUS_SUCCESS.
            ExInitializeResourceLite(self.as_mut_ptr());
        }
    }

    fn delete(&mut self) {
        unsafe {
            // As per disassembly of ntoskrnl build 22621.1 ExDeleteResource will either return
            // STATUS_SUCCESS or BugCheck/fastfail, hence ignore returned NTSTATUS.
            ExDeleteResourceLite(self.as_mut_ptr())
        };
    }

    pub fn acquired_shared(&mut self, wait: bool) -> bool {
        unsafe { ExAcquireResourceSharedLite(self.as_mut_ptr(), u8::from(wait)) == u8::from(true) }
    }

    pub fn acquire_exclusive(&mut self, wait: bool) -> bool {
        unsafe {
            ExAcquireResourceExclusiveLite(self.as_mut_ptr(), u8::from(wait)) == u8::from(true)
        }
    }

    pub fn convert_to_shared(&mut self) {
        unsafe {
            ExConvertExclusiveToSharedLite(self.as_mut_ptr());
        }
    }

    pub fn release(&mut self) {
        unsafe {
            ExReleaseResourceLite(self.as_mut_ptr());
        }
    }
}

#[cfg(feature = "lock_api")]
pub mod lock_api {
    use super::WduEResource;
    use crate::sync::{enter_critical_region, leave_critical_region};

    pub type WduEResourceRw<T> = lock_api::RwLock<WduEResource, T>;

    impl WduEResource {
        pub fn init_lock(&self) {
            let ptr = &self as *const _ as *mut Self;
            unsafe {
                (*ptr).init();
            }
        }

        pub fn lock_shared(&self) {
            let ptr = &self as *const _ as *mut Self;
            unsafe {
                // Call with WaitState to true so we put the caller into wait state
                (*ptr).acquired_shared(true);
            }
        }

        pub fn try_lock_shared(&self) -> bool {
            let ptr = &self as *const _ as *mut Self;
            unsafe {
                // Call with WaitState to false
                (*ptr).acquired_shared(false)
            }
        }

        pub unsafe fn unlock_shared(&self) {
            let ptr = &self as *const _ as *mut Self;
            (*ptr).release();
        }

        pub fn lock_exclusive(&self) {
            let ptr = &self as *const _ as *mut Self;
            unsafe {
                (*ptr).acquire_exclusive(true);
            }
        }

        pub fn try_lock_exclusive(&self) -> bool {
            let ptr = &self as *const _ as *mut Self;
            unsafe { (*ptr).acquire_exclusive(false) }
        }

        pub unsafe fn unlock_exclusive(&self) {
            let ptr = &self as *const _ as *mut Self;
            (*ptr).release();
        }
    }

    #[allow(dead_code)]
    unsafe impl lock_api::RawRwLock for WduEResource {
        type GuardMarker = lock_api::GuardSend;

        const INIT: Self = Self::const_new();

        fn lock_shared(&self) {
            enter_critical_region();
            self.lock_shared();
        }

        fn try_lock_shared(&self) -> bool {
            enter_critical_region();
            self.try_lock_shared()
        }

        unsafe fn unlock_shared(&self) {
            self.unlock_shared();
            leave_critical_region();
        }

        fn lock_exclusive(&self) {
            enter_critical_region();
            self.lock_exclusive();
        }

        fn try_lock_exclusive(&self) -> bool {
            enter_critical_region();
            self.try_lock_exclusive()
        }

        unsafe fn unlock_exclusive(&self) {
            self.unlock_exclusive();
            leave_critical_region();
        }
    }
}

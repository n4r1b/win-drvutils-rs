use crate::{
    dereference, inner_getters_ptr, nt::ExSemaphoreObjectType, ref_by_handle, ProcessorMode,
    WduResult,
};
use windows_sys::Wdk::System::SystemServices::{
    KeInitializeSemaphore, KeReadStateSemaphore, KeReleaseSemaphore, KSEMAPHORE,
};
use windows_sys::Win32::Foundation::HANDLE;

#[derive(Clone)]
pub struct WduSemaphore {
    semaphore: *mut KSEMAPHORE,
}

impl Default for WduSemaphore {
    fn default() -> Self {
        Self::new()
    }
}

inner_getters_ptr!(WduSemaphore, semaphore, KSEMAPHORE);

impl WduSemaphore {
    #[cfg(feature = "const_new")]
    pub const fn const_new() -> Self {
        WduSemaphore {
            semaphore: core::ptr::null_mut(),
        }
    }

    pub fn new() -> Self {
        WduSemaphore {
            semaphore: core::ptr::null_mut(),
        }
    }

    pub fn init(&mut self, count: i32, limit: i32) {
        unsafe {
            KeInitializeSemaphore(self.as_mut_ptr(), count, limit);
        }
    }

    pub fn release(&mut self, increment: i32, adjustment: i32, wait: bool) -> i32 {
        unsafe { KeReleaseSemaphore(self.as_mut_ptr(), increment, adjustment, u8::from(wait)) }
    }

    pub fn read_state(&mut self) -> i32 {
        unsafe { KeReadStateSemaphore(self.as_mut_ptr()) }
    }

    pub fn ref_by_handle(
        handle: HANDLE,
        access_mask: u32,
        access_mode: ProcessorMode,
    ) -> WduResult<Self> {
        let mut object = WduSemaphore::new();

        ref_by_handle(
            handle,
            access_mask,
            unsafe { Some(*ExSemaphoreObjectType) },
            access_mode,
            &mut object.semaphore,
        )?;

        Ok(object)
    }

    pub fn dereference(&self) {
        dereference(self.as_ptr() as *const _);
    }
}

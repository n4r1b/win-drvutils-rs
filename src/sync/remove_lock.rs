use crate::{inner_getters_value, io::irp::WduIrp};
use windows_sys::{
    Wdk::System::SystemServices::{
        IoAcquireRemoveLockEx, IoInitializeRemoveLockEx, IoReleaseRemoveLockAndWaitEx,
        IoReleaseRemoveLockEx, IO_REMOVE_LOCK, IO_REMOVE_LOCK_DBG_BLOCK,
    },
    Win32::Foundation::NTSTATUS,
};

#[cfg(feature = "const_new")]
use const_zero::const_zero;

// TODO: use IO_REMOVE_LOCK_DBG_BLOCK if debug
pub struct WduRemoveLock {
    lock: IO_REMOVE_LOCK,
    lock_tag: Option<usize>,
}

impl Default for WduRemoveLock {
    fn default() -> Self {
        Self::new()
    }
}

inner_getters_value!(WduRemoveLock, lock, IO_REMOVE_LOCK);

impl WduRemoveLock {
    #[cfg(feature = "const_new")]
    pub const fn const_new() -> Self {
        unsafe { const_zero!(WduRemoveLock) }
    }

    pub fn new() -> Self {
        WduRemoveLock {
            lock: unsafe { core::mem::zeroed() },
            lock_tag: None,
        }
    }

    pub fn init(&mut self, tag: u32, max_min: u32, high_water: u32) {
        unsafe {
            IoInitializeRemoveLockEx(
                self.as_mut_ptr(),
                tag,
                max_min,
                high_water,
                core::mem::size_of::<IO_REMOVE_LOCK>() as u32,
            )
        }
    }

    pub fn acquire(&mut self, tag: Option<usize>) -> NTSTATUS {
        self.lock_tag = tag;
        unsafe {
            IoAcquireRemoveLockEx(
                self.as_mut_ptr(),
                self.lock_tag
                    .map_or_else(|| core::ptr::null(), |tag| tag as *const _),
                file!().as_ptr(),
                line!(),
                core::mem::size_of::<IO_REMOVE_LOCK>() as u32,
            )
        }
    }

    pub fn release(&mut self) {
        unsafe {
            IoReleaseRemoveLockEx(
                self.as_mut_ptr(),
                self.lock_tag
                    .map_or_else(|| core::ptr::null(), |tag| tag as *const _),
                core::mem::size_of::<IO_REMOVE_LOCK>() as u32,
            )
        }
        self.lock_tag = None;
    }

    pub fn release_and_wait(&mut self) {
        unsafe {
            IoReleaseRemoveLockAndWaitEx(
                self.as_mut_ptr(),
                self.lock_tag
                    .map_or_else(|| core::ptr::null(), |tag| tag as *const _),
                core::mem::size_of::<IO_REMOVE_LOCK>() as u32,
            )
        }
        self.lock_tag = None;
    }
}

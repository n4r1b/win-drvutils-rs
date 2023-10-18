//! Collection of utils to work with kernel Synchronization primitives
use crate::ProcessorMode;
use core::borrow::Borrow;
use windows_sys::{
    Wdk::System::SystemServices::{
        KeEnterCriticalRegion, KeLeaveCriticalRegion, KeWaitForMultipleObjects,
        KeWaitForSingleObject,
    },
    Win32::Foundation::NTSTATUS,
};

pub mod eresource;
pub mod event;
pub mod mutex;
pub mod pushlock;
pub mod remove_lock;
pub mod rundown_protection;
pub mod semaphore;
pub mod spinlock;
pub mod timer;

/// Wrapper of [KeEnterCriticalRegion](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntddk/nf-ntddk-keentercriticalregion)
pub fn enter_critical_region() {
    unsafe { KeEnterCriticalRegion() }
}

/// Wrapper of [KeLeaveCriticalRegion](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntddk/nf-ntddk-keleavecriticalregion)
pub fn leave_critical_region() {
    unsafe { KeLeaveCriticalRegion() }
}

/// Wrapper of [KeWaitForSingleObject](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/wdm/nf-wdm-kewaitforsingleobject)
pub fn wait_single_object(
    object: *const u8,
    wait_reason: i32,
    mode: ProcessorMode,
    alertable: bool,
    timeout: i64,
) -> NTSTATUS {
    unsafe {
        KeWaitForSingleObject(
            object as *const _,
            wait_reason,
            mode.into(),
            u8::from(alertable),
            timeout.borrow(),
        )
    }
}

pub fn wait_multiple_objects() {
    todo!()
}

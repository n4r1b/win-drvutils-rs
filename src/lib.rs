#![no_std]
#![feature(alloc_error_handler)]
#![cfg_attr(feature = "allocator_api", feature(allocator_api))]
#![cfg_attr(feature = "allocator_api", feature(vec_into_raw_parts))]
extern crate alloc;

use crate::{alloc::string::ToString, strings::unicode::str::WduUnicodeStr};
use core::{ffi::c_void, panic::PanicInfo};
use snafu::Snafu;
use windows_sys::Wdk::System::SystemServices::{KeGetCurrentIrql, ObfDereferenceObject};
use windows_sys::Win32::Foundation::STATUS_SUCCESS;
use windows_sys::{
    Wdk::{
        Foundation::POBJECT_TYPE,
        System::SystemServices::{
            ExGetPreviousMode, KeBugCheckEx, MmGetSystemRoutineAddress, ObReferenceObjectByHandle,
            ProbeForRead, ProbeForWrite,
        },
    },
    Win32::Foundation::{HANDLE, NTSTATUS},
};

pub mod callbacks;
pub mod common;
pub mod io;
pub mod memory;
pub mod registry;
pub mod strings;
pub mod sync;

/// Base win-drvutils-rs error
#[derive(Debug, Snafu)]
pub enum WduError {
    #[snafu(display("NTSTATUS: {status}"))]
    NtStatus { status: NTSTATUS },
}

/// Base Win-driver-utils result
pub type WduResult<T> = Result<T, WduError>;

// TODO: implement option that can take generic params
macro_rules! inner_getters_value {
    ($type_name:ident, $member:ident, $inner_type:ty) => {
        impl $type_name {
            pub fn get(&self) -> $inner_type {
                self.$member
            }

            pub fn as_ref(&self) -> &$inner_type {
                &self.$member
            }

            pub fn as_ptr(&self) -> *const $inner_type {
                core::borrow::Borrow::borrow(&self.$member)
            }

            pub fn as_mut_ptr(&mut self) -> *mut $inner_type {
                core::borrow::BorrowMut::borrow_mut(&mut self.$member)
            }
        }
    };
}

macro_rules! inner_getters_ptr {
    ($type_name:ident, $member:ident, $inner_type:ty) => {
        impl $type_name {
            pub fn as_ptr(&self) -> *const $inner_type {
                self.$member
            }

            pub fn as_mut_ptr(&mut self) -> *mut $inner_type {
                self.$member
            }
        }
    };
}

pub(crate) use inner_getters_ptr;
pub(crate) use inner_getters_value;

/// Wrapper of KPROCESSOR_MODE
pub enum ProcessorMode {
    UserMode,
    KernelMode,
}

impl From<i8> for ProcessorMode {
    fn from(value: i8) -> Self {
        match value {
            0 => Self::KernelMode,
            1 => Self::UserMode,
            _ => unreachable!(),
        }
    }
}

impl Into<i8> for ProcessorMode {
    fn into(self) -> i8 {
        match self {
            ProcessorMode::KernelMode => 0,
            ProcessorMode::UserMode => 1,
        }
    }
}

impl ProcessorMode {
    /// Get previous processor mode.
    /// See [ExGetPreviousMode](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/wdm/nf-wdm-exgetpreviousmode)
    pub fn previous_mode() -> Self {
        unsafe { ExGetPreviousMode().into() }
    }
}

/// Wrapper over [MmGetSystemRoutineAddress](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/wdm/nf-wdm-mmgetsystemroutineaddress)
///
pub fn get_system_routine_addr<T>(rtn_name: &WduUnicodeStr) -> Option<T> {
    let ptr = unsafe { MmGetSystemRoutineAddress(&rtn_name.into()) as *mut u8 };

    if ptr.is_null() {
        None
    } else {
        Some(unsafe { core::mem::transmute_copy(&ptr) })
    }
}

#[inline(always)]
pub fn probe_read(addr: *const c_void, len: usize, alignment: u32) {
    unsafe {
        ProbeForRead(addr, len, alignment);
    }
}

#[inline(always)]
pub fn probe_write(addr: *mut c_void, len: usize, alignment: u32) {
    unsafe {
        ProbeForWrite(addr, len, alignment);
    }
}

#[allow(dead_code)]
pub mod nt {
    use crate::POBJECT_TYPE;

    #[link(name = "ntoskrnl")]
    extern "system" {
        pub(crate) static CmKeyObjectType: *const POBJECT_TYPE;
        pub(crate) static IoFileObjectType: *const POBJECT_TYPE;
        pub(crate) static ExEventObjectType: *const POBJECT_TYPE;
        pub(crate) static ExSemaphoreObjectType: *const POBJECT_TYPE;
        pub(crate) static TmTransactionManagerObjectType: *const POBJECT_TYPE;
        pub(crate) static TmResourceManagerObjectType: *const POBJECT_TYPE;
        pub(crate) static TmEnlistmentObjectType: *const POBJECT_TYPE;
        pub(crate) static TmTransactionObjectType: *const POBJECT_TYPE;
        pub(crate) static PsProcessType: *const POBJECT_TYPE;
        pub(crate) static PsThreadType: *const POBJECT_TYPE;
        pub(crate) static PsJobType: *const POBJECT_TYPE;
        pub(crate) static SeTokenObjectType: *const POBJECT_TYPE;

        // TODO: figure out how to do #if (NTDDI_VERSION >= NTDDI_THRESHOLD)
        pub(crate) static ExDesktopObjectType: *const POBJECT_TYPE;
    }
}

const WDU_BUGCHECK_CODE: u32 = 0x06941393;

/// Wrapper of [KeBugCheckEx]
pub fn bug_check(
    info: &PanicInfo,
    code: Option<u32>,
    param2: Option<usize>,
    param3: Option<usize>,
    param4: Option<usize>,
) {
    let msg = info.to_string();
    unsafe {
        KeBugCheckEx(
            code.map_or_else(|| WDU_BUGCHECK_CODE, |code| code),
            msg.as_ptr() as usize,
            param2.unwrap_or_default(),
            param3.unwrap_or_default(),
            param4.unwrap_or_default(),
        );
    }
}

// TODO: Consider if ref_by_handle & dereference should be public
// TODO: Create AccessMask enum
#[inline(always)]
pub(crate) fn ref_by_handle<T>(
    handle: HANDLE,
    access_mask: u32,
    obj_type: Option<POBJECT_TYPE>,
    access_mode: ProcessorMode,
    object: &mut T,
) -> WduResult<()> {
    let status = unsafe {
        ObReferenceObjectByHandle(
            handle,
            access_mask,
            obj_type.map_or_else(|| 0, |object| object),
            access_mode.into(),
            object as *mut _ as *mut _,
            core::ptr::null_mut(),
        )
    };

    if status != STATUS_SUCCESS {
        return Err(WduError::NtStatus { status });
    }

    Ok(())
}

#[inline(always)]
pub(crate) fn dereference(object: *const c_void) {
    unsafe {
        ObfDereferenceObject(object);
    }
}

pub fn current_irql() -> u8 {
    unsafe { KeGetCurrentIrql() }
}

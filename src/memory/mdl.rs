#![allow(non_snake_case)]
use crate::{io::irp::WduIrp, ProcessorMode};
use bitflags::bitflags;
use core::ffi::c_void;
use snafu::Snafu;
use windows_sys::Wdk::{
    Foundation::MDL,
    System::SystemServices::{
        IoAllocateMdl, IoFreeMdl, MmCached, MmMapLockedPagesSpecifyCache, MmProbeAndLockPages,
        MmUnlockPages,
    },
};

// We could find this constants inside "windows::Win32::Graphics::DirectDraw", but for now let's define them
const MDL_MAPPED_TO_SYSTEM_VA: u32 = 1;
const MDL_SOURCE_IS_NONPAGED_POOL: u32 = 4;

#[derive(Debug, Snafu)]
pub enum WduMdlError {
    #[snafu(display("Unable to allocate MDL"))]
    AllocateError,
}

pub type WduMdlResult<T> = Result<T, WduMdlError>;

#[derive(Clone)]
pub struct WduMdl {
    mdl: *mut MDL,
    lock: bool,
    alloc: bool,
}

pub enum LockOperation {
    IoReadAccess,
    IoWriteAccess,
    IoModifyAccess,
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PagePriority: u32 {
        const Low = 0;
        const Normal = 16;
        const High = 32;
        const MdlMappingWithGuardPtes = 0x20000000;
        const MdlMappingNoExecute = 0x40000000;
        const MdlMappingNoWrite = 0x80000000;
    }
}

impl Into<i32> for LockOperation {
    fn into(self) -> i32 {
        match self {
            Self::IoReadAccess => 0,
            Self::IoWriteAccess => 1,
            Self::IoModifyAccess => 2,
        }
    }
}

impl Drop for WduMdl {
    fn drop(&mut self) {
        if self.lock {
            self.unlock();
        }
        if self.alloc {
            self.free();
        }
    }
}

// TODO: Study if better to return Vec<u8> instead of *mut c_void
impl WduMdl {
    pub fn wrap(mdl: *mut MDL) -> Self {
        Self {
            mdl,
            lock: false,
            alloc: false,
        }
    }

    pub fn allocate(
        va: *const c_void,
        len: u32,
        secondary: bool,
        irp: Option<WduIrp>,
    ) -> WduMdlResult<Self> {
        let mdl = unsafe {
            IoAllocateMdl(
                va,
                len,
                u8::from(secondary),
                u8::from(false),
                irp.map_or_else(|| core::ptr::null_mut(), |mut irp| irp.as_mut_ptr()),
            )
        };

        if mdl.is_null() {
            return Err(WduMdlError::AllocateError);
        }

        Ok(Self {
            mdl,
            lock: false,
            alloc: true,
        })
    }

    /*
        if (Mdl->MdlFlags & (MDL_MAPPED_TO_SYSTEM_VA | MDL_SOURCE_IS_NONPAGED_POOL)) {
            return Mdl->MappedSystemVa;
        } else {
            return MmMapLockedPagesSpecifyCache(Mdl, KernelMode, MmCached, NULL, FALSE, Priority);
        }
    */
    pub fn get_system_addr(&self, priority: PagePriority) -> *mut c_void {
        if self.mdl.is_null() {
            return core::ptr::null_mut();
        }

        let mdl = unsafe { *self.mdl };

        let flags = (MDL_MAPPED_TO_SYSTEM_VA | MDL_SOURCE_IS_NONPAGED_POOL) as i16;
        if (mdl.MdlFlags & flags) != 0 {
            mdl.MappedSystemVa
        } else {
            unsafe {
                MmMapLockedPagesSpecifyCache(
                    self.mdl,
                    ProcessorMode::KernelMode.into(),
                    MmCached,
                    core::ptr::null(),
                    u32::from(false),
                    priority.bits(),
                )
            }
        }
    }

    /*
        #define MmGetMdlVirtualAddress(Mdl) ((PVOID) ((PCHAR) ((Mdl)->StartVa) + (Mdl)->ByteOffset))
    */
    pub fn get_va(&self) -> *mut c_void {
        if self.mdl.is_null() {
            return core::ptr::null_mut();
        }

        let mdl = unsafe { *self.mdl };
        mdl.StartVa.wrapping_add(mdl.ByteOffset as usize)
    }

    /*
        #define MmGetMdlBaseVa(Mdl)  ((Mdl)->StartVa)
    */
    pub fn get_base_va(&self) -> *mut c_void {
        if self.mdl.is_null() {
            return core::ptr::null_mut();
        }

        let mdl = unsafe { *self.mdl };
        mdl.StartVa
    }

    pub fn free(&self) {
        unsafe { IoFreeMdl(self.mdl) }
    }

    pub fn lock(&mut self) {}

    // TODO: Study how to replicate try/catch block
    pub fn probe_and_lock(&mut self, access_mode: ProcessorMode, lock_op: LockOperation) {
        unsafe {
            MmProbeAndLockPages(self.mdl, access_mode.into(), lock_op.into());
        }

        self.lock = true;
    }

    pub fn unlock(&mut self) {
        unsafe {
            MmUnlockPages(self.mdl);
        }
        self.lock = false;
    }

    pub fn byte_count(&self) -> usize {
        if self.mdl.is_null() {
            return 0;
        }

        unsafe { (*self.mdl).ByteCount as usize }
    }
}

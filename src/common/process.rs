use crate::{get_system_routine_addr, strings::unicode::str::WduUnicodeStr};
use core::fmt::{Debug, Display, Formatter};
use widestring::utf16str;
use windows_sys::{
    Wdk::{
        Foundation::PEPROCESS,
        Storage::FileSystem::PsGetProcessExitTime,
        System::SystemServices::{
            IoGetCurrentProcess, PsGetCurrentProcessId, PsGetProcessCreateTimeQuadPart,
            PsGetProcessExitStatus, PsGetProcessId, PsGetProcessStartKey,
        },
    },
    Win32::Foundation::{HANDLE, NTSTATUS},
};

type PsGetProtection = unsafe extern "system" fn(process: PEPROCESS) -> u8;

#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(transparent)]
pub struct WduProcess(PEPROCESS);

impl Display for WduProcess {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Debug for WduProcess {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:#X}", self.0)
    }
}

impl WduProcess {
    pub fn inner(&self) -> PEPROCESS {
        self.0
    }

    pub fn wrap(process: PEPROCESS) -> Self {
        Self(process)
    }

    pub fn current_process() -> Self {
        Self(unsafe { IoGetCurrentProcess() })
    }

    pub fn current_process_id() -> HANDLE {
        unsafe { PsGetCurrentProcessId() }
    }

    pub fn exit_time() -> i64 {
        unsafe { PsGetProcessExitTime() }
    }

    pub fn process_id(&self) -> HANDLE {
        unsafe { PsGetProcessId(self.inner()) }
    }

    pub fn create_time(&self) -> i64 {
        unsafe { PsGetProcessCreateTimeQuadPart(self.inner()) }
    }

    pub fn exit_status(&self) -> NTSTATUS {
        unsafe { PsGetProcessExitStatus(self.inner()) }
    }

    pub fn start_key(&self) -> u64 {
        unsafe { PsGetProcessStartKey(self.inner()) }
    }

    pub fn protection(&self) -> Option<u8> {
        let ps_protection_name = utf16str!("PsGetProcessProtection");
        let ps_protection = WduUnicodeStr::from_slice(ps_protection_name.as_slice());
        let pfn = get_system_routine_addr::<PsGetProtection>(&ps_protection);

        unsafe { pfn.map_or_else(|| None, |pfn| Some(pfn(self.inner()))) }
    }
}

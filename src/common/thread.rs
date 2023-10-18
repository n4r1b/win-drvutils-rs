use core::fmt::{Debug, Display, Formatter};
use windows_sys::{
    Wdk::{
        Foundation::PETHREAD,
        System::SystemServices::{
            PsGetCurrentThreadId, PsGetThreadCreateTime, PsGetThreadProcessId,
        },
    },
    Win32::Foundation::HANDLE,
};

#[repr(transparent)]
pub struct WduThread(PETHREAD);

impl Display for WduThread {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:#X}", self.0)
    }
}

impl Debug for WduThread {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self)
    }
}

impl WduThread {
    pub fn inner(&self) -> PETHREAD {
        self.0
    }

    pub fn wrap(thread: PETHREAD) -> Self {
        Self(thread)
    }

    pub fn current_thread_id() -> HANDLE {
        unsafe { PsGetCurrentThreadId() }
    }

    pub fn process_id(&self) -> HANDLE {
        unsafe { PsGetThreadProcessId(self.inner()) }
    }

    pub fn create_time(&self) -> i64 {
        unsafe { PsGetThreadCreateTime(self.inner()) }
    }
}

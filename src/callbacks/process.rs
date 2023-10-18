use crate::{
    callbacks::{WduCallbackError, WduCallbackResult},
    common::process::WduProcess,
    io::file_obj::WduFileObject,
    WduUnicodeStr,
};
use windows_sys::{
    Wdk::System::SystemServices::{
        PsCreateProcessNotifySubsystems, PsSetCreateProcessNotifyRoutine,
        PCREATE_PROCESS_NOTIFY_ROUTINE, PSCREATEPROCESSNOTIFYTYPE, PS_CREATE_NOTIFY_INFO,
    },
    Win32::{
        Foundation::{HANDLE, NTSTATUS, STATUS_SUCCESS},
        System::WindowsProgramming::CLIENT_ID,
    },
};

#[repr(transparent)]
pub struct WduPsCreateNotifyInfo(*mut PS_CREATE_NOTIFY_INFO);

impl WduPsCreateNotifyInfo {
    pub fn is_exit(&self) -> bool {
        self.0.is_null()
    }

    fn get(&self) -> &PS_CREATE_NOTIFY_INFO {
        unsafe { &(*self.0) }
    }

    fn get_mut(&self) -> &mut PS_CREATE_NOTIFY_INFO {
        unsafe { &mut (*self.0) }
    }

    pub fn set_status(&mut self, status: NTSTATUS) {
        self.get_mut().CreationStatus = status;
    }

    pub fn parent_pid(&self) -> HANDLE {
        self.get().ParentProcessId
    }

    pub fn cmdline(&self) -> WduUnicodeStr {
        WduUnicodeStr::from_ptr(self.get().CommandLine)
    }

    pub fn image_filename(&self) -> WduUnicodeStr {
        WduUnicodeStr::from_ptr(self.get().ImageFileName)
    }

    pub fn fileobj(&self) -> WduFileObject {
        WduFileObject::wrap(self.get().FileObject)
    }

    // TODO: Wrap ClientId
    pub fn client_id(&self) -> CLIENT_ID {
        self.get().CreatingThreadId
    }

    pub fn open_name_available(&self) -> bool {
        unsafe { (self.get().Anonymous.Flags & 1) == 1 }
    }
}

pub type PsNotifyRoutineEx =
    unsafe extern "system" fn(WduProcess, HANDLE, WduPsCreateNotifyInfo) -> ();

// As per the definition of PsSetCreateProcessNotifyRoutineEx2, the NotifyInformation is a PVOID.
// Since at the moment the only option is for it to be of type PCREATE_PROCESS_NOTIFY_ROUTINE_EX,
// let's override the windows-sys with our own prototype and we will define our own PCREATE_PROCESS_NOTIFY_ROUTINE_EX
// with our own newtypes.
mod nt {
    #[link(name = "ntoskrnl")]
    extern "system" {
        pub(crate) fn PsSetCreateProcessNotifyRoutineEx(
            notify_routine: super::PsNotifyRoutineEx,
            remove: bool,
        ) -> super::NTSTATUS;

        pub(crate) fn PsSetCreateProcessNotifyRoutineEx2(
            notify_type: super::PSCREATEPROCESSNOTIFYTYPE,
            notify_information: super::PsNotifyRoutineEx,
            remove: bool,
        ) -> super::NTSTATUS;
    }
}

pub enum PsCallbackVersion {
    NotifyRoutine(PCREATE_PROCESS_NOTIFY_ROUTINE),
    NotifyRoutineEx(PsNotifyRoutineEx),
    // TODO: Consider NotifyType, since there's only one at the moment hardcode to PsCreateNotifyEx
    NotifyRoutineEx2(PsNotifyRoutineEx),
}

#[derive(Default)]
pub struct WduPsCallback {
    callback: Option<PsCallbackVersion>,
}

impl Drop for WduPsCallback {
    fn drop(&mut self) {
        // TODO: Find a better option than ignoring error. Failing to unregister during Drop
        // is a critical error so maybe panic would be the best option.
        let _ = self.unregister();
    }
}

impl WduPsCallback {
    #[cfg(feature = "const_new")]
    pub const fn const_new() -> Self {
        Self { callback: None }
    }

    unsafe fn control_callbacks(callback: &PsCallbackVersion, remove: bool) -> NTSTATUS {
        match callback {
            PsCallbackVersion::NotifyRoutine(cb) => {
                PsSetCreateProcessNotifyRoutine(*cb, u8::from(remove))
            }
            PsCallbackVersion::NotifyRoutineEx(cb) => {
                nt::PsSetCreateProcessNotifyRoutineEx(*cb, remove)
            }
            PsCallbackVersion::NotifyRoutineEx2(cb) => {
                nt::PsSetCreateProcessNotifyRoutineEx2(PsCreateProcessNotifySubsystems, *cb, remove)
            }
        }
    }

    pub fn register(&mut self, callback: PsCallbackVersion) -> WduCallbackResult<()> {
        let status = unsafe { Self::control_callbacks(&callback, false) };

        if status != STATUS_SUCCESS {
            return Err(WduCallbackError::RegisterError { status });
        }

        self.callback = Some(callback);
        Ok(())
    }

    pub fn unregister(&mut self) -> WduCallbackResult<()> {
        if self.callback.is_none() {
            return Err(WduCallbackError::NotRegistered);
        }

        let status = unsafe { Self::control_callbacks(&self.callback.as_ref().unwrap(), true) };

        if status != STATUS_SUCCESS {
            return Err(WduCallbackError::UnregisterError { status });
        }

        self.callback = None;

        Ok(())
    }
}

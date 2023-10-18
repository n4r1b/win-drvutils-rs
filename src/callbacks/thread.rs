use crate::callbacks::{WduCallbackError, WduCallbackResult};
use windows_sys::{
    Wdk::System::SystemServices::{
        PsRemoveCreateThreadNotifyRoutine, PsSetCreateThreadNotifyRoutine,
        PsSetCreateThreadNotifyRoutineEx, PCREATE_THREAD_NOTIFY_ROUTINE, PSCREATETHREADNOTIFYTYPE,
    },
    Win32::Foundation::STATUS_SUCCESS,
};

pub enum ThCallbackVersion {
    NotifyRoutine(PCREATE_THREAD_NOTIFY_ROUTINE),
    NotifyRoutineEx((PSCREATETHREADNOTIFYTYPE, PCREATE_THREAD_NOTIFY_ROUTINE)),
}

pub struct WduThCallback {
    callback: Option<ThCallbackVersion>,
}

impl Drop for WduThCallback {
    fn drop(&mut self) {
        // TODO: Find a better option than ignoring error. Failing to unregister during Drop
        // is a critical error so maybe panic would be the best option.
        let _ = self.unregister();
    }
}

impl WduThCallback {
    #[cfg(feature = "const_new")]
    pub const fn const_new() -> Self {
        Self { callback: None }
    }

    pub fn register(&mut self, callback: ThCallbackVersion) -> WduCallbackResult<()> {
        let status = unsafe {
            match &callback {
                ThCallbackVersion::NotifyRoutine(cb) => PsSetCreateThreadNotifyRoutine(*cb),
                ThCallbackVersion::NotifyRoutineEx((notify_type, cb)) => {
                    PsSetCreateThreadNotifyRoutineEx(notify_type.clone(), core::mem::transmute(*cb))
                }
            }
        };

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

        let cb = match self.callback.as_ref().unwrap() {
            ThCallbackVersion::NotifyRoutine(cb) | ThCallbackVersion::NotifyRoutineEx((_, cb)) => {
                cb
            }
        };

        let status = unsafe { PsRemoveCreateThreadNotifyRoutine(*cb) };

        if status != STATUS_SUCCESS {
            return Err(WduCallbackError::UnregisterError { status });
        }

        self.callback = None;

        Ok(())
    }
}

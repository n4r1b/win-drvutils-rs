use crate::callbacks::{WduCallbackError, WduCallbackResult};
use windows_sys::{
    Wdk::System::SystemServices::{
        PsRemoveLoadImageNotifyRoutine, PsSetLoadImageNotifyRoutine, PLOAD_IMAGE_NOTIFY_ROUTINE,
    },
    Win32::Foundation::STATUS_SUCCESS,
};

#[derive(Default)]
pub struct WduImageCallback {
    callback: PLOAD_IMAGE_NOTIFY_ROUTINE,
}

impl Drop for WduImageCallback {
    fn drop(&mut self) {
        // TODO: Find a better option than ignoring error. Failing to unregister during Drop
        // is a critical error so maybe panic would be the best option.
        let _ = self.unregister();
    }
}

impl WduImageCallback {
    #[cfg(feature = "const_new")]
    pub const fn const_new() -> Self {
        Self { callback: None }
    }

    pub fn register(&mut self, callback: PLOAD_IMAGE_NOTIFY_ROUTINE) -> WduCallbackResult<()> {
        assert!(callback.is_some());

        let status = unsafe { PsSetLoadImageNotifyRoutine(callback) };

        if status != STATUS_SUCCESS {
            return Err(WduCallbackError::RegisterError { status });
        }

        self.callback = callback;

        Ok(())
    }

    pub fn unregister(&mut self) -> WduCallbackResult<()> {
        if self.callback.is_none() {
            return Err(WduCallbackError::NotRegistered);
        }

        let status = unsafe { PsRemoveLoadImageNotifyRoutine(self.callback) };

        if status != STATUS_SUCCESS {
            return Err(WduCallbackError::UnregisterError { status });
        }

        self.callback = None;

        Ok(())
    }
}

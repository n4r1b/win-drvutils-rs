use crate::{
    callbacks::{WduCallbackError, WduCallbackResult},
    strings::unicode::string::WduUnicodeString,
};
use core::{borrow::BorrowMut, ffi::c_void};
use windows_sys::{
    Wdk::System::SystemServices::{
        CmCallbackGetKeyObjectIDEx, CmCallbackReleaseKeyObjectIDEx, CmGetBoundTransaction,
        CmGetCallbackVersion, CmRegisterCallbackEx, CmUnRegisterCallback,
    },
    Win32::Foundation::{NTSTATUS, STATUS_SUCCESS},
};

#[derive(Default)]
pub struct CmCallbackVersion {
    minor: u32,
    major: u32,
}

#[derive(Default)]
pub struct WduCmCallback {
    cookie: i64,
    altitude: WduUnicodeString,
}

#[derive(Default)]
pub struct WduCmKeyObject {
    object_id: usize,
    object_name: WduUnicodeString,
}

impl Drop for WduCmKeyObject {
    fn drop(&mut self) {
        unsafe {
            CmCallbackReleaseKeyObjectIDEx(self.object_name.as_ptr());
        }
    }
}

// TODO
// pub type ExCallbackFunction = Fn(c_void, )

unsafe extern "system" fn test() -> NTSTATUS {
    STATUS_SUCCESS
}

impl WduCmCallback {
    pub fn altitude(&mut self, altitude: WduUnicodeString) {
        self.altitude = altitude
    }

    pub fn register(&mut self) -> WduCallbackResult<()> {
        let status = unsafe {
            CmRegisterCallbackEx(
                Some(test),
                self.altitude.as_ptr(),
                core::ptr::null(),
                core::ptr::null(),
                self.cookie.borrow_mut(),
                core::ptr::null(),
            )
        };

        if status != STATUS_SUCCESS {
            return Err(WduCallbackError::RegisterError { status });
        }

        Ok(())
    }

    pub fn unregister(&mut self) -> WduCallbackResult<()> {
        let status = unsafe { CmUnRegisterCallback(self.cookie) };

        if status != STATUS_SUCCESS {
            return Err(WduCallbackError::NtStatus { status });
        }

        Ok(())
    }

    pub fn version() -> CmCallbackVersion {
        let mut version = CmCallbackVersion::default();
        unsafe {
            CmGetCallbackVersion(&mut version.major, &mut version.minor);
        }
        version
    }

    // TODO: Return ObjectId & ObjectName
    pub fn key_object_id(&mut self, object: &c_void) -> WduCallbackResult<WduCmKeyObject> {
        let mut key_object = WduCmKeyObject::default();

        let status = unsafe {
            CmCallbackGetKeyObjectIDEx(
                &self.cookie,
                object,
                &mut key_object.object_id,
                &mut key_object.object_name.as_mut_ptr(),
                0,
            )
        };

        if status != STATUS_SUCCESS {
            return Err(WduCallbackError::NtStatus { status });
        }

        Ok(key_object)
    }

    pub fn bound_transaction(&self, object: &c_void) {
        unsafe {
            // TODO: pass object
            CmGetBoundTransaction(&self.cookie, object);
        }
    }
}

use crate::{
    callbacks::{WduCallbackError, WduCallbackResult},
    common::obj_attr::WduObjectAttributes,
};
use core::{
    borrow::BorrowMut,
    ffi::c_void,
    ops::{Deref, DerefMut},
};
use windows_sys::{
    Wdk::{
        Foundation::PCALLBACK_OBJECT,
        System::SystemServices::{ExCreateCallback, ExNotifyCallback, ExUnregisterCallback},
    },
    Win32::Foundation::{STATUS_SUCCESS, STATUS_UNSUCCESSFUL},
};

// windows-sys declares the CALLBACK_FUNCTION as `fn() -> ()`. Let's redefine it with the proper
// type:
// typedef
// VOID
// CALLBACK_FUNCTION (
//     _In_opt_ PVOID CallbackContext,
//     _In_opt_ PVOID Argument1,
//     _In_opt_ PVOID Argument2
//     );
type CallbackFunction = unsafe extern "system" fn(*const c_void, *mut c_void, *mut c_void) -> ();

// We need to update the function ExRegisterCallback to use our CallbackFunction prototype. Let's
// link and define a new prototype for the function.
// Keeping redefined prototypes in the `nt` modules to avoid confusion.
mod nt {
    #[link(name = "ntoskrnl")]
    extern "system" {
        pub(crate) fn ExRegisterCallback(
            callbackobject: super::PCALLBACK_OBJECT,
            callbackfunction: super::CallbackFunction,
            callbackcontext: *const super::c_void,
        ) -> *mut super::c_void;
    }
}

struct CallbackHandle(*mut c_void);

impl Default for CallbackHandle {
    fn default() -> Self {
        Self(core::ptr::null_mut())
    }
}

impl Deref for CallbackHandle {
    type Target = *mut c_void;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for CallbackHandle {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub type WduCallbackFunction<T> = fn(&T, *mut c_void, *mut c_void) -> ();

pub struct WduCallbackObject<'a, T> {
    object: PCALLBACK_OBJECT,
    handle: CallbackHandle,
    ctx: &'a T,
    cb: Option<WduCallbackFunction<T>>,
}

impl<'a, T> Drop for WduCallbackObject<'a, T> {
    fn drop(&mut self) {
        self.unregister();
    }
}

impl<'a, T: 'a> WduCallbackObject<'a, T> {
    fn new(ctx: &'a T) -> Self {
        Self {
            object: PCALLBACK_OBJECT::default(),
            handle: CallbackHandle::default(),
            ctx,
            cb: None,
        }
    }

    pub fn open(obj_attr: &WduObjectAttributes, ctx: &'a T) -> WduCallbackResult<Self> {
        let mut cb_obj = Self::new(ctx);

        let status = unsafe {
            ExCreateCallback(
                cb_obj.object.borrow_mut(),
                obj_attr.as_ptr(),
                u8::from(false),
                u8::from(false),
            )
        };

        if status != STATUS_SUCCESS {
            return Err(WduCallbackError::CreateError { status });
        }

        Ok(cb_obj)
    }

    pub fn create(
        allow_multiple: bool,
        obj_attr: &WduObjectAttributes,
        ctx: &'a T,
    ) -> WduCallbackResult<Self> {
        let mut cb_obj = Self::new(ctx);

        let status = unsafe {
            ExCreateCallback(
                cb_obj.object.borrow_mut(),
                obj_attr.as_ptr(),
                u8::from(true),
                u8::from(allow_multiple),
            )
        };

        if status != STATUS_SUCCESS {
            return Err(WduCallbackError::CreateError { status });
        }

        Ok(cb_obj)
    }

    pub fn register(&mut self, cb: WduCallbackFunction<T>) -> WduCallbackResult<()> {
        let handle = unsafe {
            let ptr = core::mem::transmute(&self);
            nt::ExRegisterCallback(self.object, Self::cb_internal, ptr)
        };

        if handle.is_null() {
            return Err(WduCallbackError::RegisterError {
                status: STATUS_UNSUCCESSFUL,
            });
        }

        self.cb = Some(cb);
        self.handle = CallbackHandle(handle);

        Ok(())
    }

    pub fn unregister(&mut self) {
        unsafe {
            ExUnregisterCallback(*self.handle);
            self.cb = None;
        }
    }

    unsafe extern "system" fn cb_internal(
        ctx: *const c_void,
        arg1: *mut c_void,
        arg2: *mut c_void,
    ) {
        unsafe {
            let ptr = core::mem::transmute::<*const c_void, *const Self>(ctx);
            (*ptr)
                .cb
                .map_or_else(|| (), |pfn| pfn((*ptr).ctx, arg1, arg2))
        }
    }

    pub fn notify(&self, arg1: *mut c_void, arg2: *mut c_void) {
        unsafe {
            let ptr: *const c_void = self.object as *mut c_void;
            ExNotifyCallback(ptr, arg1, arg2);
        }
    }
}

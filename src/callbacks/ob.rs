use crate::{
    callbacks::{WduCallbackError, WduCallbackResult},
    nt,
    strings::unicode::{
        string::WduUnicodeString,
        str::WduUnicodeStr
    },
};
use alloc::{boxed::Box, vec::Vec};
use core::ffi::c_void;
use windows_sys::{
    Wdk::Foundation::POBJECT_TYPE,
    Wdk::System::SystemServices::{
        ObGetFilterVersion, ObRegisterCallbacks, ObUnRegisterCallbacks, OB_CALLBACK_REGISTRATION,
        OB_FLT_REGISTRATION_VERSION, OB_OPERATION_HANDLE_CREATE, OB_OPERATION_HANDLE_DUPLICATE,
        OB_OPERATION_REGISTRATION, OB_POST_OPERATION_INFORMATION, OB_POST_OPERATION_PARAMETERS,
        OB_PREOP_CALLBACK_STATUS, OB_PREOP_SUCCESS, OB_PRE_OPERATION_INFORMATION,
        OB_PRE_OPERATION_PARAMETERS, POB_POST_OPERATION_CALLBACK, POB_PRE_OPERATION_CALLBACK,
    },
    Win32::Foundation::{
        NTSTATUS, STATUS_INTERNAL_ERROR, STATUS_INVALID_PARAMETER, STATUS_SUCCESS,
    },
};

// mut pointer because we can modifify DesiredAccess & set CallContext
#[repr(transparent)]
#[derive(Debug, Copy, Clone)]
pub struct ObPreOpInfo(*mut OB_PRE_OPERATION_INFORMATION);

// const pointer since members of this structure are only informational, not modifiable
#[repr(transparent)]
#[derive(Debug, Copy, Clone)]
pub struct ObPostOpInfo(*const OB_POST_OPERATION_INFORMATION);

pub type ObPreOpCb<T> = fn(Option<&T>, ObPreOpInfo) -> ();
pub type ObPostOpCb<T> = fn(Option<&T>, ObPostOpInfo) -> ();

impl ObPreOpInfo {
    fn get(&self) -> &OB_PRE_OPERATION_INFORMATION {
        unsafe { &(*self.0) }
    }

    fn get_mut(&mut self) -> &mut OB_PRE_OPERATION_INFORMATION {
        unsafe { &mut (*self.0) }
    }

    fn get_params(&self) -> &OB_PRE_OPERATION_PARAMETERS {
        unsafe { &(*self.get().Parameters) }
    }

    fn get_params_mut(&mut self) -> &mut OB_PRE_OPERATION_PARAMETERS {
        unsafe { &mut (*self.get_mut().Parameters) }
    }

    #[cfg(not(feature = "allocator_api"))]
    pub fn set_context<T>(&mut self, context: T) {
        // Filter manager sets this value to NULL
        assert!(self.get().CallContext.is_null());
        self.get_mut().CallContext = Box::into_raw(Box::new(context)) as _;
    }

    // TODO: Use Box::try_new
    #[cfg(feature = "allocator_api")]
    pub fn set_context<T>(&mut self, _context: T) -> WduCallbackResult<()> {
        todo!()
    }

    pub fn operation(&self) -> u32 {
        self.get().Operation
    }

    pub fn object(&self) -> *mut c_void {
        self.get().Object
    }

    pub fn object_type(&self) -> ObjectType {
        unsafe {
            let object_type = self.get().ObjectType;

            if object_type == (*nt::PsProcessType) {
                ObjectType::Process
            } else if object_type == (*nt::PsThreadType) {
                ObjectType::Thread
            } else if object_type == (*nt::ExDesktopObjectType) {
                ObjectType::Desktop
            } else {
                ObjectType::Unknown
            }
        }
    }

    pub fn desired_access(&self) -> u32 {
        unsafe {
            match self.get().Operation {
                OB_OPERATION_HANDLE_CREATE => {
                    self.get_params().CreateHandleInformation.DesiredAccess
                }
                OB_OPERATION_HANDLE_DUPLICATE => {
                    self.get_params().DuplicateHandleInformation.DesiredAccess
                }
                _ => unreachable!(),
            }
        }
    }

    pub fn original_desired_access(&self) -> u32 {
        unsafe {
            match self.get().Operation {
                OB_OPERATION_HANDLE_CREATE => {
                    self.get_params()
                        .CreateHandleInformation
                        .OriginalDesiredAccess
                }
                OB_OPERATION_HANDLE_DUPLICATE => {
                    self.get_params()
                        .CreateHandleInformation
                        .OriginalDesiredAccess
                }
                _ => unreachable!(),
            }
        }
    }

    pub fn set_desired_access(&mut self, desired_access: u32) {
        unsafe {
            let access = match self.get().Operation {
                OB_OPERATION_HANDLE_CREATE => {
                    &mut self.get_params_mut().CreateHandleInformation.DesiredAccess
                }
                OB_OPERATION_HANDLE_DUPLICATE => {
                    &mut self
                        .get_params_mut()
                        .DuplicateHandleInformation
                        .DesiredAccess
                }
                _ => unreachable!(),
            };

            *access = desired_access;
        }
    }

    pub fn is_kernel_handle(&self) -> bool {
        unsafe { (self.get().Anonymous.Flags & 1) == 1 }
    }
}

impl ObPostOpInfo {
    fn get(&self) -> &OB_POST_OPERATION_INFORMATION {
        unsafe { &(*self.0) }
    }

    fn get_params(&self) -> &OB_POST_OPERATION_PARAMETERS {
        unsafe { &(*self.get().Parameters) }
    }

    pub fn call_context<T>(&self) -> Option<Box<T>> {
        if self.get().CallContext.is_null() {
            return None;
        }
        unsafe { Some(Box::from_raw(self.get().CallContext as *mut _)) }
    }

    pub fn operation(&self) -> u32 {
        self.get().Operation
    }

    pub fn return_status(&self) -> NTSTATUS {
        self.get().ReturnStatus
    }

    pub fn object(&self) -> *mut c_void {
        self.get().Object
    }

    pub fn object_type(&self) -> ObjectType {
        unsafe {
            let object_type = self.get().ObjectType;

            if object_type == (*nt::PsProcessType) {
                ObjectType::Process
            } else if object_type == (*nt::PsThreadType) {
                ObjectType::Thread
            } else if object_type == (*nt::ExDesktopObjectType) {
                ObjectType::Desktop
            } else {
                ObjectType::Unknown
            }
        }
    }

    pub fn granted_access(&self) -> u32 {
        unsafe {
            match self.get().Operation {
                OB_OPERATION_HANDLE_CREATE => {
                    self.get_params().CreateHandleInformation.GrantedAccess
                }
                OB_OPERATION_HANDLE_DUPLICATE => {
                    self.get_params().DuplicateHandleInformation.GrantedAccess
                }
                _ => unreachable!(),
            }
        }
    }

    pub fn is_kernel_handle(&self) -> bool {
        unsafe { (self.get().Anonymous.Flags & 1) == 1 }
    }
}

#[derive(Default, Debug, Copy, Clone, PartialEq)]
pub enum ObjectType {
    #[default]
    Unknown,
    Process,
    Thread,
    Desktop,
}

impl Into<*const POBJECT_TYPE> for ObjectType {
    fn into(self) -> *const POBJECT_TYPE {
        unsafe {
            match self {
                Self::Process => nt::PsProcessType,
                Self::Thread => nt::PsThreadType,
                Self::Desktop => nt::ExDesktopObjectType,
                Self::Unknown => unreachable!(),
            }
        }
    }
}

// TODO: Study if possible to store a reference for the context. Hard to deal with lifetimes when
//  this data is stored and passed to us by the OS.
pub struct WduObOpRegistration<T> {
    ob_type: ObjectType,
    operations: u32,
    pre: Option<ObPreOpCb<T>>,
    post: Option<ObPostOpCb<T>>,
    context: *mut T,
}

impl<T> Default for WduObOpRegistration<T> {
    fn default() -> Self {
        Self {
            ob_type: Default::default(),
            operations: 0,
            pre: None,
            post: None,
            context: core::ptr::null_mut(),
        }
    }
}

impl<T> WduObOpRegistration<T> {
    pub fn ob_type(mut self, ob_type: ObjectType) -> Self {
        self.ob_type = ob_type;
        self
    }

    pub fn operations(mut self, operations: u32) -> Self {
        self.operations = operations;
        self
    }

    pub fn pre(mut self, pre: ObPreOpCb<T>) -> Self {
        self.pre = Some(pre);
        self
    }

    pub fn post(mut self, post: ObPostOpCb<T>) -> Self {
        self.post = Some(post);
        self
    }

    pub fn build(self) -> Self {
        self
    }

    pub fn set_context(mut self, context: &T) -> Self {
        self.context = context as *const T as *mut _;
        self
    }

    fn into_native(&self) -> OB_OPERATION_REGISTRATION {
        let object_type: *const POBJECT_TYPE = self.ob_type.into();
        let mut pre: POB_PRE_OPERATION_CALLBACK = None;
        let mut post: POB_POST_OPERATION_CALLBACK = None;

        if self.pre.is_some() {
            pre = Some(WduObCallback::<T>::pre_op_internal);
        }
        if self.post.is_some() {
            post = Some(WduObCallback::<T>::post_op_internal);
        }

        OB_OPERATION_REGISTRATION {
            ObjectType: object_type as *mut _,
            Operations: self.operations,
            PreOperation: pre,
            PostOperation: post,
        }
    }
}

pub struct WduObCallback<T> {
    handle: *mut c_void,
    operations: Vec<WduObOpRegistration<T>>,
}

impl<T> Default for WduObCallback<T> {
    fn default() -> Self {
        Self {
            handle: core::ptr::null_mut(),
            operations: Vec::new(),
        }
    }
}

impl<T> WduObCallback<T> {
    #[cfg(feature = "const_new")]
    pub const fn const_new() -> Self {
        Self {
            handle: core::ptr::null_mut(),
            operations: Vec::new(),
        }
    }

    pub fn push_op_registration(&mut self, op_registration: WduObOpRegistration<T>) {
        // If we have a try_push at some point prefer over this to avoid OOM condition.
        // Consider using crate fallible_vec!
        self.operations.push(op_registration);
    }

    pub fn register(&mut self, altitude: WduUnicodeStr) -> WduCallbackResult<()> {
        if self.operations.is_empty() {
            return Err(WduCallbackError::NtStatus {
                status: STATUS_INVALID_PARAMETER,
            });
        }

        let mut operations = Vec::new();
        operations
            .try_reserve(self.operations.len())
            .or_else(|_| Err(WduCallbackError::InsufficientResources))?;

        operations.extend(self.operations.iter().map(|op| op.into_native()));

        let callback_reg = OB_CALLBACK_REGISTRATION {
            Version: OB_FLT_REGISTRATION_VERSION as u16,
            OperationRegistrationCount: self.operations.len() as u16,
            Altitude: altitude.as_unicode_string(),
            RegistrationContext: self as *const _ as *mut _,
            OperationRegistration: operations.as_mut_ptr(),
        };

        let status = unsafe { ObRegisterCallbacks(&callback_reg, &mut self.handle) };

        if status != STATUS_SUCCESS {
            return Err(WduCallbackError::RegisterError { status });
        }

        Ok(())
    }

    pub fn unregister(&mut self) -> WduCallbackResult<()> {
        if self.handle.is_null() {
            return Err(WduCallbackError::UnregisterError {
                status: STATUS_INTERNAL_ERROR,
            });
        }

        unsafe { ObUnRegisterCallbacks(self.handle) };

        self.handle = core::ptr::null_mut();

        Ok(())
    }

    fn get_self<'a>(context: *const c_void) -> &'a Self {
        unsafe { core::mem::transmute(context) }
    }

    // #[no_mangle]
    unsafe extern "system" fn pre_op_internal(
        context: *const c_void,
        op_info: *mut OB_PRE_OPERATION_INFORMATION,
    ) -> OB_PREOP_CALLBACK_STATUS {
        let ob_cb: &WduObCallback<T> = WduObCallback::get_self(context);
        let op_data = ObPreOpInfo(op_info);

        ob_cb
            .operations
            .iter()
            .filter(|op| {
                op.ob_type == op_data.object_type()
                    && (op.operations & op_data.operation()) == op_data.operation()
                    && op.pre.is_some()
            })
            .for_each(|op| {
                let pre_fn = op.pre.unwrap();

                if op.context.is_null() {
                    pre_fn(None, op_data);
                } else {
                    pre_fn(Some(&mut *op.context), op_data);
                }
            });

        OB_PREOP_SUCCESS
    }

    // #[no_mangle]
    unsafe extern "system" fn post_op_internal(
        context: *const c_void,
        op_info: *const OB_POST_OPERATION_INFORMATION,
    ) {
        let ob_cb: &WduObCallback<T> = WduObCallback::get_self(context);
        let op_data = ObPostOpInfo(op_info);

        ob_cb
            .operations
            .iter()
            .filter(|op| {
                op.ob_type == op_data.object_type()
                    && (op.operations & op_data.operation()) == op_data.operation()
                    && op.post.is_some()
            })
            .for_each(|op| {
                let post_fn = op.post.unwrap();
                if op.context.is_null() {
                    post_fn(None, op_data);
                } else {
                    post_fn(Some(&mut *op.context), op_data);
                }
            });
    }
}

impl WduObCallback<()> {
    pub fn version() -> u16 {
        unsafe { ObGetFilterVersion() }
    }
}

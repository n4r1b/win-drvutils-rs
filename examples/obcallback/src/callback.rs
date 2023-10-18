use crate::{ObCallbackError, ObCallbackResult};

use alloc::boxed::Box;
use log::{error, info};
use widestring::utf16str;

use win_drvutils_rs::{
    callbacks::ob::{ObPostOpInfo, ObPreOpInfo, ObjectType, WduObCallback, WduObOpRegistration},
    common::{
        thread::WduThread,
        process::WduProcess
    },
    io::device_control::WduDeviceControl
};
use win_drvutils_rs::{
    strings::unicode::{
        str::WduUnicodeStr,
        string::WduUnicodeString
    },
};
use windows_sys::{
    Wdk::System::SystemServices::{
        OB_OPERATION_HANDLE_CREATE, OB_OPERATION_HANDLE_DUPLICATE,
    },
    Win32::Foundation::{HANDLE, NTSTATUS, STATUS_NOT_FOUND, STATUS_SUCCESS},
};

pub(crate) static mut PROTECT_NAME_FLAG: bool = false;
pub(crate) static mut REJECT_NAME_FLAG: bool = false;

// Could be an enum, we could then implement From/TryFrom and transform from the value we recieve
// in ProtectNameInput::operation
const PROTECT_NAME_PROTECT: u32 = 0;
const PROTECT_NAME_REJECT: u32 = 1;

const CB_PROCESS_TERMINATE: u32 = 0x0001;
const CB_THREAD_TERMINATE: u32 = 0x0001;

const NAME_SIZE: usize = 200;

#[repr(C)]
pub(crate) struct ProtectNameInput {
    operation: u32,
    name: [u16; NAME_SIZE + 1],
}

#[allow(dead_code)]
#[repr(C)]
pub(crate) struct UnprotectCallbackInput {
    unused: u32,
}

#[derive(Debug)]
enum Object {
    Process(WduProcess),
    Thread(HANDLE),
}

#[derive(Debug)]
struct CallContext {
    operation: u32,
    object: Object,
}

#[derive(Clone)]
pub(crate) struct ProtectData {
    target_process: Option<WduProcess>,
    target_process_id: Option<HANDLE>,
    name: WduUnicodeString,
}

impl ProtectData {
    const fn const_new() -> Self {
        Self {
            target_process: None,
            target_process_id: None,
            name: WduUnicodeString::const_new(),
        }
    }
}

pub(crate) struct Protect {
    callback_installed: bool,
    protect_data: ProtectData,
    callback: WduObCallback<ProtectData>,
}

impl Protect {
    pub(crate) const fn const_new() -> Protect {
        Self {
            callback_installed: false,
            protect_data: ProtectData::const_new(),
            callback: WduObCallback::const_new(),
        }
    }

    pub(crate) fn is_cb_installed(&self) -> bool {
        self.callback_installed
    }

    pub(crate) fn protect_name_cb(
        &mut self,
        req_data: &WduDeviceControl,
    ) -> ObCallbackResult<NTSTATUS> {
        let input_buffer = req_data.input_buffer() as *const ProtectNameInput;

        if input_buffer.is_null() {
            return Err(ObCallbackError::InvalidParameter);
        }

        self.protect_data.name =
            WduUnicodeString::try_from(unsafe { (*input_buffer).name.as_slice() })?;

        self.register_callback()?;

        unsafe {
            match (*input_buffer).operation {
                PROTECT_NAME_PROTECT => {
                    PROTECT_NAME_FLAG = true;
                    REJECT_NAME_FLAG = false;
                }
                PROTECT_NAME_REJECT => {
                    PROTECT_NAME_FLAG = false;
                    REJECT_NAME_FLAG = true;
                }
                _ => return Err(ObCallbackError::InvalidParameter),
            }
        }

        Ok(STATUS_SUCCESS)
    }

    fn register_callback(&mut self) -> ObCallbackResult<()> {
        let process_cb = WduObOpRegistration::default()
            .ob_type(ObjectType::Process)
            .operations(OB_OPERATION_HANDLE_CREATE | OB_OPERATION_HANDLE_DUPLICATE)
            .pre(Self::pre_operation_cb)
            .post(Self::post_operation_cb)
            .set_context(&self.protect_data)
            .build();

        let thread_cb = WduObOpRegistration::default()
            .ob_type(ObjectType::Thread)
            .operations(OB_OPERATION_HANDLE_CREATE | OB_OPERATION_HANDLE_DUPLICATE)
            .pre(Self::pre_operation_cb)
            .post(Self::post_operation_cb)
            .set_context(&self.protect_data)
            .build();

        self.callback.push_op_registration(process_cb);
        self.callback.push_op_registration(thread_cb);

        let altitude_utf16 = utf16str!("1000");
        let altitude = WduUnicodeStr::from_slice(altitude_utf16.as_slice());

        self.callback.register(altitude)?;
        self.callback_installed = true;

        Ok(())
    }

    pub(crate) fn delete_protect_name_cb(&mut self) -> ObCallbackResult<()> {
        self.callback.unregister()?;
        self.callback_installed = false;
        Ok(())
    }

    pub(crate) fn check_process_match(
        &mut self,
        process: WduProcess,
        pid: HANDLE,
        filename: &WduUnicodeString, // MS example uses the CmdLine for simplicity we will use the image_filename
    ) -> NTSTATUS {
        // We could return a result here tbh

        if !filename.contains(&self.protect_data.name, true) {
            return STATUS_NOT_FOUND;
        }

        // We might want to synchronize access, even thou callbacks will only read this value
        // and this function is the only writer.
        self.protect_data.target_process = Some(process);
        self.protect_data.target_process_id = Some(pid);

        STATUS_SUCCESS
    }

    fn pre_operation_cb(ctx: Option<&ProtectData>, mut op_info: ObPreOpInfo) {
        let object;
        let clear_bit_access;
        let set_bit_access;

        if ctx.is_none() {
            info!("Ignore ObCallback with no ProtectData");
            return;
        }

        let protect_data = ctx.unwrap();

        match op_info.object_type() {
            ObjectType::Process => {
                let process = WduProcess::wrap(op_info.object() as _);

                //
                // Ignore requests if:
                // - target_process is None
                // - processes other than our target process.
                //
                if protect_data
                    .target_process
                    .map_or_else(|| true, |protected_process| protected_process != process)
                {
                    return;
                }

                if process == WduProcess::current_process() {
                    info!("Ignore thread open/duplicate from the protected process itself");
                    return;
                }

                object = Object::Process(process);
                clear_bit_access = CB_PROCESS_TERMINATE;
                set_bit_access = 0;
            }
            ObjectType::Thread => {
                let target_thread = WduThread::wrap(op_info.object() as _);
                let pid = target_thread.process_id();
                //
                // Ignore requests if:
                // - target_process_id is None
                // - threads belonging to processes other than our target process.
                //
                if protect_data
                    .target_process_id
                    .map_or_else(|| true, |protected_pid| protected_pid != pid)
                {
                    return;
                }

                //
                // Also ignore requests for threads belonging to the current processes.
                //
                if pid == WduProcess::current_process_id() {
                    info!("Ignore thread open/duplicate from the protected process itself");
                    return;
                }

                object = Object::Thread(pid);
                clear_bit_access = CB_THREAD_TERMINATE;
                set_bit_access = 0;
            }
            _ => {
                error!("Unexpected object type");
                return;
            }
        }

        let mut desired_access = op_info.desired_access();

        // Filter only if request made outside of the kernel
        if !op_info.is_kernel_handle() {
            desired_access &= !clear_bit_access;
            desired_access |= set_bit_access;
            op_info.set_desired_access(desired_access);
        }

        // Set Context
        let call_ctx = CallContext {
            operation: op_info.operation(),
            object,
        };

        op_info.set_context(call_ctx);

        // TODO: Should we implement Display/Debug for ObPreOpInfo ???
        info!(
            "PreOperation Callback\n\
        \tClientId: {}:{}\n\
        \tObject: {:?}\n\
        \tType: {:?}\n\
        \tOperation: {} (KernelHandle={})\n\
        \tOriginalDesiredAccess: {:x}\n\
        \tDesiredAccess(in): {:x}\n\
        \tDesiredAccess(out): {:x}",
            WduProcess::current_process_id(),
            WduThread::current_thread_id(),
            op_info.object(),
            op_info.object_type(),
            op_info.operation(), // returns u32, in the MS example they print a Name
            op_info.is_kernel_handle(),
            op_info.original_desired_access(),
            op_info.desired_access(),
            desired_access,
        );
    }

    fn post_operation_cb(_ctx: Option<&ProtectData>, op_info: ObPostOpInfo) {
        // CallContext, if any, will be droped by the end of the function
        let call_ctx: Option<Box<CallContext>> = op_info.call_context();

        call_ctx.map_or_else(
            || return,
            |ctx| {
                assert!(ctx.operation == op_info.operation());

                info!("CallContext data: {:?}", ctx);

                match ctx.object {
                    Object::Process(target_process) => {
                        //
                        // Ignore requests for processes other than our target process.
                        //
                        if target_process == WduProcess::wrap(op_info.object() as _) {
                            return;
                        }

                        //
                        // Ignore requests that are trying to open/duplicate the current process.
                        //
                        if target_process == WduProcess::current_process() {
                            return;
                        }
                    }
                    Object::Thread(target_process_id) => {
                        let process_of_target_thread =
                            WduThread::wrap(op_info.object() as _).process_id();

                        //
                        // Ignore requests for threads belonging to processes other than our
                        // target process.
                        //
                        if target_process_id == process_of_target_thread {
                            return;
                        }

                        //
                        // Ignore requests for threads belonging to the current processes.
                        //
                        if target_process_id == WduProcess::current_process_id() {
                            return;
                        }
                    }
                }
            },
        )
    }
}

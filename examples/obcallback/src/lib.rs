#![no_std]
#![allow(internal_features)]
#![feature(lang_items)]
extern crate alloc;
mod callback;

use kernel_log::KernelLogger;
use log::{error, info, warn, LevelFilter};
use widestring::{utf16str, Utf16Str};

use crate::callback::Protect;
use win_drvutils_rs::{
    callbacks::{
        ob::WduObCallback,
        process::{PsCallbackVersion, WduPsCallback, WduPsCreateNotifyInfo},
        WduCallbackError,
    },
    common::{
        driver::{FileObjDispatch, IoDispath, WduDriver, WduDriverError},
        process::WduProcess,
    },
    encode_ioctl,
    io::{
        create::WduCreate,
        device::{WduDevice, WduDeviceChars, WduDeviceError, WduDeviceType},
        device_control::WduDeviceControl,
        irp::{WduIoStatus, WduIrp},
        file_obj::WduFileObject
    },
    memory::pool::SimpleAlloc,
    strings::unicode::{
        str::WduUnicodeStr,
        WduUnicodeError,
    },
    sync::mutex::{lock_api::WduGuardedMtx, WduGuardedMutex},
};
use windows_sys::{
    Wdk::Foundation::DRIVER_OBJECT,
    Win32::{
        Foundation::{HANDLE, NTSTATUS, STATUS_ACCESS_DENIED, STATUS_INVALID_DEVICE_REQUEST,
                     STATUS_SUCCESS, STATUS_UNSUCCESSFUL, UNICODE_STRING},
        System::Ioctl::{
            FILE_DEVICE_UNKNOWN, FILE_SPECIAL_ACCESS, METHOD_BUFFERED,
        }
    },
};

#[global_allocator]
static mut GLOBAL: SimpleAlloc = SimpleAlloc::const_new();

#[export_name = "_fltused"]
static _FLTUSED: i32 = 0;

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[lang = "eh_personality"]
extern "C" fn eh_personality() {}

#[no_mangle]
extern "system" fn __CxxFrameHandler3(_: *mut u8, _: *mut u8, _: *mut u8, _: *mut u8) -> i32 {
    unimplemented!()
}

#[derive(Debug)]
pub enum ObCallbackError {
    DriverError,
    DeviceError,
    CallbackError, // We could add a ctx to say if Ob or Ps callback is failing
    InvalidParameter,
    UnicodeError,
}

pub type ObCallbackResult<T> = Result<T, ObCallbackError>;

// Just for the sake of the example, this should be handled better on a real scenario
impl From<WduDeviceError> for ObCallbackError {
    fn from(_error: WduDeviceError) -> Self {
        ObCallbackError::DeviceError
    }
}

impl From<WduDriverError> for ObCallbackError {
    fn from(_error: WduDriverError) -> Self {
        ObCallbackError::DriverError
    }
}

impl From<WduCallbackError> for ObCallbackError {
    fn from(_error: WduCallbackError) -> Self {
        ObCallbackError::CallbackError
    }
}

impl From<WduUnicodeError> for ObCallbackError {
    fn from(_error: WduUnicodeError) -> Self {
        ObCallbackError::UnicodeError
    }
}

// Global variables. We don't have much of an option other than store certain variables as mut in
// the global state.
static mut PROCESS_CB: WduPsCallback = WduPsCallback::const_new();
static mut PROTECT: WduGuardedMtx<Protect> =
    WduGuardedMtx::const_new(WduGuardedMutex::const_new(), Protect::const_new());

const DEVICE_NAME_UTF16: &Utf16Str = utf16str!(r"\Device\ObCallbackTest");
const WIN32_NAME_UTF16: &Utf16Str = utf16str!(r"\DosDevices\ObCallbackTest");

const TD_IOCTL_PROTECT_NAME_CALLBACK: u32 = encode_ioctl!(
    FILE_DEVICE_UNKNOWN,
    (0x800 + 2),
    METHOD_BUFFERED,
    FILE_SPECIAL_ACCESS
);
const TD_IOCTL_UNPROTECT_CALLBACK: u32 = encode_ioctl!(
    FILE_DEVICE_UNKNOWN,
    (0x800 + 3),
    METHOD_BUFFERED,
    FILE_SPECIAL_ACCESS
);

#[no_mangle]
unsafe extern "system" fn process_notify_cb(
    process: WduProcess,
    process_id: HANDLE,
    mut create_info: WduPsCreateNotifyInfo,
) {
    if create_info.is_exit() {
        info!("Process {} (ID: {:#X}) destroyed", process, process_id);
        return;
    }

    let image_filename = create_info.image_filename().to_owned().unwrap_or_default();
    info!(
        "Process {} (ID: {:#X}) created. Creator {:x}:{:x}.\n\
            \tFileName {} (FileOpenNameAvailable {})\n\
            \tCmdLine {}",
        process,
        process_id,
        create_info.client_id().UniqueProcess,
        create_info.client_id().UniqueThread,
        image_filename,
        create_info.open_name_available(),
        create_info.cmdline(),
    );

    if callback::PROTECT_NAME_FLAG && !image_filename.is_empty() {
        let mut lock = PROTECT.lock();
        let _ = lock.check_process_match(process, process_id, &image_filename);
    }

    if callback::REJECT_NAME_FLAG && !image_filename.is_empty() {
        let mut lock = PROTECT.lock();
        if lock.check_process_match(process, process_id, &image_filename) == STATUS_SUCCESS {
            create_info.set_status(STATUS_ACCESS_DENIED);
        }
    }
}

#[no_mangle]
pub extern "system" fn DriverEntry(
    driver_object: *mut DRIVER_OBJECT,
    _registry_path: *const UNICODE_STRING,
) -> NTSTATUS {
    KernelLogger::init(LevelFilter::Info).expect("Failed to initialize logger");

    unsafe {
        // Initialize GlobalAllocator
        GLOBAL.init();

        // Initialize the Guarded mutex
        PROTECT.raw().init_lock();
    }

    info!("Callback version {:#X}", WduObCallback::version());

    if let Err(err) = init(driver_object) {
        error!("Error initializing {:?}", err);
        return STATUS_UNSUCCESSFUL;
    }

    STATUS_SUCCESS
}

fn init(driver_object: *mut DRIVER_OBJECT) -> ObCallbackResult<()> {
    let device_name = WduUnicodeStr::from_slice(DEVICE_NAME_UTF16.as_slice());
    let win32_name = WduUnicodeStr::from_slice(WIN32_NAME_UTF16.as_slice());

    // ObCallbackVersion
    let fileobj = FileObjDispatch::default()
        .create_irp(create)
        .close_irp(close)
        .cleanup_irp(cleanup);

    let io = IoDispath::default().ioctl_irp(device_control);

    let wdu_driver = WduDriver::new(driver_object)
        .unload(driver_unload)
        .file_object(fileobj)
        .io(io)
        .build()?;

    let wdu_device = WduDevice::default()
        .device_type(WduDeviceType::Unknown)
        .characteristics(WduDeviceChars::SecureOpen)
        .build::<()>(&wdu_driver, Some(&device_name))?; // Zero Sized type for the Device Extension

    if let Err(err) = wdu_device.symbolic_name(&device_name, &win32_name) {
        error!("Error creating symbolic name. {:?}", err);
        wdu_device.delete();
    }

    if let Err(err) =
        unsafe { PROCESS_CB.register(PsCallbackVersion::NotifyRoutineEx2(process_notify_cb)) }
    {
        error!("Error registering PsNotifyRoutine. {:?}", err);

        // Cleanup, ignore errors
        let _ = WduDevice::delete_symbolic_name(&win32_name);
        wdu_device.delete();
    }

    Ok(())
}

fn driver_unload(driver: &WduDriver) {
    let dev_obj = driver.device();
    let win32_name = WduUnicodeStr::from_slice(WIN32_NAME_UTF16.as_slice());

    // Unsafe required due to static mut.
    unsafe {
        // ignore error
        let _ = PROCESS_CB.unregister();

        // Acquire Guarded Mutex
        let mut lock = PROTECT.lock();
        if lock.is_cb_installed() {
            if let Err(err) = lock.delete_protect_name_cb() {
                warn!("Error deleting protect name callback: {:?}", err);
            }
        }
    }

    if let Err(err) = WduDevice::delete_symbolic_name(&win32_name) {
        error!("Error creating deleting symbolic name. {:?}", err);
    }

    dev_obj.delete();
}

fn create(_device: &WduDevice, request: &mut WduIrp, _req_data: WduCreate) {
    let io_status = WduIoStatus::success_no_info();
    request.complete(io_status);
}

fn close(_device: &WduDevice, request: &mut WduIrp, _file_obj: WduFileObject) {
    let io_status = WduIoStatus::success_no_info();
    request.complete(io_status);
}

fn cleanup(_device: &WduDevice, request: &mut WduIrp, _file_obj: WduFileObject) {
    let io_status = WduIoStatus::success_no_info();
    request.complete(io_status);
}

fn device_control(_device: &WduDevice, request: &mut WduIrp, req_data: WduDeviceControl) -> NTSTATUS {
    // Init io_status with STATUS_UNSUCCESSFUL;
    let mut io_status = WduIoStatus::new();
    let ioctl = req_data.ioctl();

    let status = match ioctl {
        TD_IOCTL_PROTECT_NAME_CALLBACK => protect_name(&req_data).unwrap_or(STATUS_UNSUCCESSFUL),
        TD_IOCTL_UNPROTECT_CALLBACK => unprotect().unwrap_or(STATUS_UNSUCCESSFUL),
        _ => {
            warn!("Invalid request. IOCTL: {}", ioctl);
            STATUS_INVALID_DEVICE_REQUEST
        }
    };

    io_status.set_status(status);
    request.complete(io_status);

    status
}

fn protect_name(req_data: &WduDeviceControl) ->  ObCallbackResult<NTSTATUS> {
    let in_size = req_data.input_buffer_size();
    if in_size < core::mem::size_of::<callback::ProtectNameInput>() {
        return Err(ObCallbackError::InvalidParameter);
    }

    unsafe {
        let mut lock = PROTECT.lock();
        // Acquire Guarded Mutex
        if !lock.is_cb_installed() {
            lock.protect_name_cb(req_data)?;
        }
    }

    Ok(STATUS_SUCCESS)
}

fn unprotect() -> ObCallbackResult<NTSTATUS> {
    unsafe {
        // Acquire Guarded Mutex
        let mut lock = PROTECT.lock();
        if lock.is_cb_installed() {
            lock.delete_protect_name_cb()?;
        }
    }

    Ok(STATUS_SUCCESS)
}

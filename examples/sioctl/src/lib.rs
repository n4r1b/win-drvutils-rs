#![no_std]
#![allow(internal_features)]
#![feature(lang_items)]
extern crate alloc;

use alloc::slice;
use kernel_log::KernelLogger;
use log::{error, info, warn, LevelFilter};
use widestring::{utf16str, Utf16Str};

use win_drvutils_rs::{
    common::driver::{FileObjDispatch, IoDispath, WduDriver, WduDriverError},
    encode_ioctl,
    io::{
        create::WduCreate,
        device::{WduDevice, WduDeviceChars, WduDeviceError, WduDeviceType},
        device_control::WduDeviceControl,
        irp::{WduIoStatus, WduIrp},
        file_obj::WduFileObject
    },
    memory::{
        pool::SimpleAlloc,
        mdl::{LockOperation, PagePriority, WduMdl, WduMdlError}
    },
    strings::unicode::str::WduUnicodeStr,
    ProcessorMode,
};

use windows_sys::{
    Wdk::Foundation::DRIVER_OBJECT,
    Win32::{
        Foundation::{
            NTSTATUS, STATUS_INSUFFICIENT_RESOURCES, STATUS_INVALID_DEVICE_REQUEST,
            STATUS_INVALID_PARAMETER, STATUS_SUCCESS, STATUS_UNSUCCESSFUL, UNICODE_STRING,
        },
        System::Ioctl::{
            FILE_ANY_ACCESS, METHOD_BUFFERED, METHOD_IN_DIRECT, METHOD_NEITHER, METHOD_OUT_DIRECT,
        }
    }
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
pub enum SioctlError {
    DriverError,
    DeviceError,
    MdlError,
}

pub type SioctlResult<T> = Result<T, SioctlError>;

// Just for the sake of the example, this should be handled better on a real scenario
impl From<WduDeviceError> for SioctlError {
    fn from(_error: WduDeviceError) -> Self {
        SioctlError::DeviceError
    }
}

impl From<WduDriverError> for SioctlError {
    fn from(_error: WduDriverError) -> Self {
        SioctlError::DriverError
    }
}

impl From<WduMdlError> for SioctlError {
    fn from(_error: WduMdlError) -> Self {
        SioctlError::MdlError
    }
}

const DEVICE_NAME_UTF16: &Utf16Str = utf16str!(r"\Device\SIOCTL");
const WIN32_NAME_UTF16: &Utf16Str = utf16str!(r"\DosDevices\IoctlTest");

const SIOCTL_TYPE: u32 = 40000;
const IOCTL_SIOCTL_METHOD_IN_DIRECT: u32 =
    encode_ioctl!(SIOCTL_TYPE, 0x900, METHOD_IN_DIRECT, FILE_ANY_ACCESS);
const IOCTL_SIOCTL_METHOD_OUT_DIRECT: u32 =
    encode_ioctl!(SIOCTL_TYPE, 0x901, METHOD_OUT_DIRECT, FILE_ANY_ACCESS);
const IOCTL_SIOCTL_METHOD_BUFFERED: u32 =
    encode_ioctl!(SIOCTL_TYPE, 0x902, METHOD_BUFFERED, FILE_ANY_ACCESS);
const IOCTL_SIOCTL_METHOD_NEITHER: u32 =
    encode_ioctl!(SIOCTL_TYPE, 0x903, METHOD_NEITHER, FILE_ANY_ACCESS);

#[no_mangle]
pub extern "system" fn DriverEntry(
    driver_object: *mut DRIVER_OBJECT,
    _registry_path: *const UNICODE_STRING,
) -> NTSTATUS {
    KernelLogger::init(LevelFilter::Info).expect("Failed to initialize logger");

    unsafe {
        GLOBAL.init();
    }

    if let Err(err) = init(driver_object) {
        error!("Error initializing {:?}", err);
        return STATUS_UNSUCCESSFUL;
    }

    STATUS_SUCCESS
}

fn init(driver_object: *mut DRIVER_OBJECT) -> SioctlResult<()> {
    let device_name = WduUnicodeStr::from_slice(DEVICE_NAME_UTF16.as_slice());
    let win32_name = WduUnicodeStr::from_slice(WIN32_NAME_UTF16.as_slice());

    let fileobj = FileObjDispatch::default()
        .create_irp(create)
        .close_irp(close);
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

    Ok(())
}

fn driver_unload(driver: &WduDriver) {
    let dev_obj = driver.device();
    let win32_name = WduUnicodeStr::from_slice(WIN32_NAME_UTF16.as_slice());

    if let Err(err) = WduDevice::delete_symbolic_name(&win32_name) {
        error!("Error deleting symbolic name. {:?}", err);
    }

    dev_obj.delete();
}

fn create(_device: &WduDevice, request: &mut WduIrp, _req_data: WduCreate) {
    let io_status = WduIoStatus::success_no_info();
    request.complete(io_status);
}

fn close(_device: &WduDevice, request: &mut WduIrp, _file_object: WduFileObject) {
    let io_status = WduIoStatus::success_no_info();
    request.complete(io_status);
}

fn device_control(_device: &WduDevice, request: &mut WduIrp, req_data: WduDeviceControl) -> NTSTATUS {
    let data = "String from Rust Device Driver!";

    // Init io_status to STATUS_SUCCES and Information == 0;
    let mut io_status = WduIoStatus::success_no_info();

    let ioctl = req_data.ioctl();
    let in_buf_size = req_data.input_buffer_size();
    let out_buf_size = req_data.output_buffer_size();

    if in_buf_size == 0 || out_buf_size == 0 {
        io_status.set_status(STATUS_INVALID_PARAMETER);
        request.complete(io_status);

        return STATUS_INVALID_PARAMETER;
    }

    // WduDeviceControl knows from where to retrieve the buffer so we can do this here regardless
    // of the I/O buffer method
    let in_buf = req_data.input_buffer();
    let out_buf = req_data.output_buffer();

    if out_buf.is_null() {
        io_status.set_status(STATUS_INSUFFICIENT_RESOURCES);
        request.complete(io_status);

        return STATUS_INSUFFICIENT_RESOURCES;
    }

    let status = match ioctl {
        // MS example splits this two methods to retrieve the output_buffer from different areas
        // in our case we can reuse the code since WduDeviceControl already knows from where to
        // retrieve the data
        IOCTL_SIOCTL_METHOD_BUFFERED | IOCTL_SIOCTL_METHOD_OUT_DIRECT => {
            let in_data: &[u8] = unsafe { slice::from_raw_parts(in_buf as *const _, in_buf_size) };

            // info!("Data from user: {}", core::str::from_utf8(in_data).unwrap());
            info!("Data from user: {:?}", in_data);

            unsafe {
                out_buf.copy_from(data.as_ptr() as *const _, data.len());
            }

            let out_data: &[u8] =
                unsafe { slice::from_raw_parts(out_buf as *const _, out_buf_size) };

            info!("Data to user: {:?}", out_data);

            let written_bytes = if out_buf_size < data.len() {
                out_buf_size
            } else {
                data.len()
            };

            io_status.set_info(written_bytes);
            STATUS_SUCCESS
        }
        IOCTL_SIOCTL_METHOD_IN_DIRECT => {
            let in_data: &[u8] = unsafe { slice::from_raw_parts(in_buf as *const _, in_buf_size) };

            info!("Data from user InputBuffer: {:?}", in_data);

            unsafe {
                out_buf.copy_from(data.as_ptr() as *const _, data.len());
            }

            let out_data: &[u8] =
                unsafe { slice::from_raw_parts(out_buf as *const _, out_buf_size) };

            info!("Data from user in OutputBuffer: {:?}", out_data);
            io_status.set_info(req_data.mdl_byte_count());
            STATUS_SUCCESS
        }
        IOCTL_SIOCTL_METHOD_NEITHER => {
            if let Ok(written_bytes) = method_neither(&req_data) {
                io_status.set_info(written_bytes);
                STATUS_SUCCESS
            } else {
                error!("Error handling Method neither");
                // Just for the sake of simplicity, set to insufficient resources
                STATUS_INSUFFICIENT_RESOURCES
            }
        }
        _ => {
            warn!("Invalid request. IOCTL: {}", ioctl);
            STATUS_INVALID_DEVICE_REQUEST
        }
    };

    io_status.set_status(status);
    request.complete(io_status);

    return status;
}

fn method_neither(req_data: &WduDeviceControl) -> SioctlResult<usize> {
    // ******* REMARK **********
    // At the moment not try/catch block alike to probe the buffers so this buffering method is
    // very unsafe. Adding the code just to demonstrate how to work with MDLs using the
    // win-drvutils-rs, but this still requies research & work.
    let data = "String from Rust Device Driver (Method Neither)!";

    {
        let mut in_mdl = WduMdl::allocate(
            req_data.input_buffer(),
            req_data.input_buffer_size() as u32,
            false,
            None,
        )?;

        // probe_and_lock doesn't implement any try/catch mechanism at the moment, this is very unsafe.
        in_mdl.probe_and_lock(ProcessorMode::UserMode, LockOperation::IoReadAccess);
        let in_buf = in_mdl.get_system_addr(PagePriority::Normal | PagePriority::MdlMappingNoExecute);

        if in_buf.is_null() {
            return Err(SioctlError::MdlError);
        }

        let in_data: &[u8] =
            unsafe { slice::from_raw_parts(in_buf as *const _, in_mdl.byte_count() as usize) };

        info!("Data from User (SystemAddress): {:?}", in_data);
    } // On Drop MDL will be unlocked and freed

    {
        let mut out_mdl = WduMdl::allocate(
            req_data.output_buffer(),
            req_data.output_buffer_size() as u32,
            false,
            None,
        )?;

        // probe_and_lock doesn't implement any try/catch mechanism at the moment, this is very unsafe.
        out_mdl.probe_and_lock(ProcessorMode::UserMode, LockOperation::IoWriteAccess);
        let out_buf = out_mdl.get_system_addr(PagePriority::Normal | PagePriority::MdlMappingNoExecute);

        if out_buf.is_null() {
            return Err(SioctlError::MdlError);
        }

        unsafe {
            out_buf.copy_from(data.as_ptr() as *const _, data.len());
        }

        let written_bytes = if out_mdl.byte_count() < data.len() {
            out_mdl.byte_count()
        } else {
            data.len()
        };

        Ok(written_bytes)
    }
}

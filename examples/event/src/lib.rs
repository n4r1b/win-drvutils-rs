#![no_std]
#![allow(internal_features)]
#![feature(lang_items)]
extern crate alloc;

use alloc::{boxed::Box, vec::Vec};
use core::ops::Neg;

use kernel_log::KernelLogger;
use log::{error, info, LevelFilter};
use widestring::{u16cstr, U16CStr};

use win_drvutils_rs::{
    common::{
        dpc::WduDpc,
        driver::{FileObjDispatch, IoDispath, WduDriver, WduDriverError},
    },
    encode_ioctl,
    io::{
        create::WduCreate,
        device::{WduDevice, WduDeviceChars, WduDeviceError, WduDeviceType},
        device_control::WduDeviceControl,
        file_obj::WduFileObject,
        irp::{WduIoStatus, WduIrp},
    },
    memory::pool::SimpleAlloc,
    strings::unicode::{str::WduUnicodeStr, WduUnicodeError},
    sync::{event::WduEvent, remove_lock::WduRemoveLock, spinlock::WduSpinLock, timer::WduTimer},
    WduError,
};

use windows_sys::{
    Wdk::{Foundation::DRIVER_OBJECT, Storage::FileSystem::DO_BUFFERED_IO},
    Win32::{
        Foundation::{
            HANDLE, NTSTATUS, STATUS_CANCELLED, STATUS_INVALID_PARAMETER, STATUS_NOT_IMPLEMENTED,
            STATUS_PENDING, STATUS_SUCCESS, STATUS_UNSUCCESSFUL, UNICODE_STRING,
        },
        System::Ioctl::{FILE_ANY_ACCESS, FILE_DEVICE_UNKNOWN, METHOD_BUFFERED},
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

#[derive(Debug, PartialEq)]
pub enum EventError {
    DriverError,
    DeviceError,
    UnicodeError,
    Status(NTSTATUS),
}

pub type EventResult<T> = Result<T, EventError>;

// Just for the sake of the example, this should be handled better on a real scenario
impl From<WduDeviceError> for EventError {
    fn from(_error: WduDeviceError) -> Self {
        EventError::DeviceError
    }
}

impl From<WduDriverError> for EventError {
    fn from(_error: WduDriverError) -> Self {
        EventError::DriverError
    }
}

impl From<WduUnicodeError> for EventError {
    fn from(_error: WduUnicodeError) -> Self {
        EventError::UnicodeError
    }
}

impl From<WduError> for EventError {
    fn from(value: WduError) -> Self {
        match value {
            WduError::NtStatus { status } => EventError::Status(status),
        }
    }
}

const LOCK_TAG: u32 = u32::from_ne_bytes(*b"TEVE");
const DEVICE_NAME_UTF16: &U16CStr = u16cstr!(r"\Device\Event_Sample");
const WIN32_NAME_UTF16: &U16CStr = u16cstr!(r"\DosDevices\Event_Sample");

const IRP_BASED: u32 = 0;
const EVENT_BASED: u32 = 1;

// We can derive clone since both WduIrp and WduEvent store pointers
#[derive(Clone)]
enum NotifyType {
    IrpBased(WduIrp),
    EventBased(WduEvent),
}

#[repr(C)]
struct RegisterEvent {
    notify_type: u32,
    event: HANDLE,
    due_time: i64,
}

const IOCTL_REGISTER_EVENT: u32 =
    encode_ioctl!(FILE_DEVICE_UNKNOWN, 0x800, METHOD_BUFFERED, FILE_ANY_ACCESS);

// Ideally we would put this into an Arc so we can share it between threads
// Just for the sake of keeping the example simple we will just work with Box and raw pointers
type EventRecord = Box<NotifyRecord>;
type EventQueue = Vec<EventRecord>;
type EventDpc = WduDpc<NotifyData, (), ()>; // Context, Arg1, Arg2

#[derive(Clone)]
struct NotifyData{
    ty: NotifyType,
    extension: *mut DeviceExtension,
}

impl PartialEq for NotifyData {
    fn eq(&self, other: &Self) -> bool {
        match &self.ty {
            NotifyType::IrpBased(irp) => {
                if let NotifyType::IrpBased(other_irp) = &other.ty {
                    other_irp.as_ptr() == irp.as_ptr()
                } else {
                    false
                }
            },
            NotifyType::EventBased(event) => {
                if let NotifyType::EventBased(other_event) = &other.ty {
                    other_event.as_ptr() == event.as_ptr()
                } else {
                    false
                }
            }
        }
    }
}

struct NotifyRecord {
    dpc: EventDpc,
    timer: WduTimer,
    file_object: WduFileObject,
    data: NotifyData,
}

#[derive(Default)]
struct FileContext {
    file_rundown: WduRemoveLock,
}

struct DeviceExtension {
    event_queue: EventQueue,
    queue_lock: WduSpinLock,
}

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

fn init(driver_object: *mut DRIVER_OBJECT) -> EventResult<()> {
    let device_name = WduUnicodeStr::from_slice(DEVICE_NAME_UTF16.as_slice());

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

    // Mutable so we can set the flag DO_BUFFERED_IO
    let mut wdu_device = WduDevice::default()
        .device_type(WduDeviceType::Unknown)
        .characteristics(WduDeviceChars::SecureOpen)
        .build::<DeviceExtension>(&wdu_driver, Some(&device_name))?;

    let win32_name = WduUnicodeStr::from_slice(WIN32_NAME_UTF16.as_slice());

    if let Err(err) = wdu_device.symbolic_name(&device_name, &win32_name) {
        error!("Error creating symbolic name. {:?}", err);
        wdu_device.delete();
    }

    // Set-up device extension
    let extension: &mut DeviceExtension = wdu_device.extension_as_mut_ref();
    extension.queue_lock = WduSpinLock::default(); // Default will initialize the spinlock!
    extension.event_queue = Vec::default();

    wdu_device.set_flags(DO_BUFFERED_IO);

    Ok(())
}

fn driver_unload(driver: &WduDriver) {
    let dev_obj = driver.device();
    let extension: DeviceExtension = dev_obj.extension();

    if !extension.event_queue.is_empty() {
        assert!(false, "Event Queue is not empty");
    }

    let win32_name = WduUnicodeStr::from_slice(WIN32_NAME_UTF16.as_slice());

    if let Err(err) = WduDevice::delete_symbolic_name(&win32_name) {
        error!("Error deleting symbolic name. {:?}", err);
    }

    dev_obj.delete();
}

fn create(_device: &WduDevice, request: &mut WduIrp, mut req_data: WduCreate) {
    info!("==> EventCreate");
    let io_status = WduIoStatus::success_no_info();

    // We could do this using default since WduRemoveLock implements default. Explicit for the
    // sake of the example.
    // let file_ctx = FileContext::default();
    let mut remove_lock = WduRemoveLock::new();
    remove_lock.init(LOCK_TAG, 0, 0);
    let file_ctx = FileContext {
        file_rundown: remove_lock,
    };

    // Make sure nobody is using the FsContext scratch area.
    assert!(req_data.file_object().context::<()>().is_none());
    req_data.file_object_mut().set_context(file_ctx);

    request.complete(io_status);
}

fn close(_device: &WduDevice, request: &mut WduIrp, file_obj: WduFileObject) {
    info!("==> EventClose");

    let io_status = WduIoStatus::success_no_info();

    if let Some(_ctx) = file_obj.context::<FileContext>() {
        info!("Freeing FsContext");
    } // If we had anything allocated inside FileContext it would be dropped here :)

    request.complete(io_status);
}

fn cleanup(device: &WduDevice, request: &mut WduIrp, file_obj: WduFileObject) {
    info!("==> EventCleanup");

    let extension: &mut DeviceExtension = device.extension_as_mut_ref();

    assert!(file_obj.is_valid());
    let fs_ctx: &mut FileContext = file_obj.context_as_mut_ref().unwrap();

    let tag = Some(request.as_ptr() as usize);
    let status = fs_ctx.file_rundown.acquire(tag);
    assert!(status == STATUS_SUCCESS);

    fs_ctx.file_rundown.release_and_wait();

    let mut cleanup_list = Vec::new();

    extension.queue_lock.acquire();

    extension.event_queue.retain_mut(|x| {
        if x.file_object != file_obj && !x.timer.cancel() {
            true
        } else {
            info!("\tCanceled timer!");
            let mut retain = false;
            match &mut x.data.ty {
                NotifyType::IrpBased(ref mut irp) => {
                    if irp.set_cancel_rtn(None).is_none() {
                        cleanup_list.push(irp.clone());
                    } else {
                        // The I/O manager cleared it and called the cancel-routine.
                        // Cancel routine is probably waiting to acquire the lock.
                        // Cancel routine will get free the entry
                        retain = true;
                    }
                }
                NotifyType::EventBased(evt) => {
                    evt.dereference();
                }
            }

            retain
        }
    });

    extension.queue_lock.release();

    cleanup_list.iter_mut().for_each(|irp| {
        let mut io_status = WduIoStatus::new();
        io_status.set_status(STATUS_CANCELLED);
        irp.complete(io_status);
    });

    let io_status = WduIoStatus::success_no_info();
    request.complete(io_status);
}

fn device_control(
    device: &WduDevice,
    request: &mut WduIrp,
    req_data: WduDeviceControl,
) -> NTSTATUS {
    info!("==> EventDispatchIoControl");
    // Init io_status to STATUS_UNSUCCESSFUL and Information == 0;
    let mut io_status = WduIoStatus::new();

    let fo = request.file_object();
    assert!(fo.is_valid());

    // Should be safe to directly uwnrap
    let fs_ctx: &mut FileContext = fo.context_as_mut_ref().unwrap();

    if fs_ctx.file_rundown.acquire(Some(request.as_ptr() as usize)) != STATUS_SUCCESS {
        request.complete(io_status);
        return STATUS_UNSUCCESSFUL;
    }

    let status = match req_data.ioctl() {
        IOCTL_REGISTER_EVENT => match handle_register_event(device, request, &req_data) {
            Ok(status) => status,
            Err(err) => match err {
                EventError::Status(status) => status,
                _ => STATUS_UNSUCCESSFUL,
            },
        },
        _ => {
            debug_assert!(false);
            STATUS_NOT_IMPLEMENTED
        }
    };

    if status != STATUS_PENDING {
        io_status.set_status(status);
        request.complete(io_status);
    }

    fs_ctx.file_rundown.release();

    status
}

fn handle_register_event(
    device: &WduDevice,
    request: &mut WduIrp,
    req_data: &WduDeviceControl,
) -> EventResult<NTSTATUS> {
    if req_data.input_buffer_size() < core::mem::size_of::<RegisterEvent>() {
        return Err(EventError::Status(STATUS_INVALID_PARAMETER));
    }

    let buffer = req_data.input_buffer() as *const RegisterEvent;
    if buffer.is_null() {
        return Err(EventError::Status(STATUS_INVALID_PARAMETER));
    }

    unsafe {
        match (*buffer).notify_type {
            IRP_BASED => irp_notification(device, request, &*buffer),
            EVENT_BASED => event_notification(device, request, &*buffer),
            _ => {
                error!("\tUnknow notification type from user-mode");
                Err(EventError::Status(STATUS_INVALID_PARAMETER))
            }
        }
    }
}

fn irp_notification(
    device: &WduDevice,
    irp: &mut WduIrp,
    register_event: &RegisterEvent,
) -> EventResult<NTSTATUS> {
    info!("\tRegisterIrpBasedNotification");
    let extension: &mut DeviceExtension = device.extension_as_mut_ref();

    // Set cancel_rtn here so we have the routine when we clone. I don't like this very much :(
    irp.set_cancel_rtn(Some(cancel_routine));

    let data = NotifyData{
        ty: NotifyType::IrpBased(irp.clone()),
        extension: device.extension_as_mut_ptr(),
    };

    let mut record = Box::new(NotifyRecord {
        file_object: irp.file_object(),
        timer: WduTimer::default(),
        dpc: WduDpc::new(),
        data: data.clone()
    });

    let mut due_time = register_event.due_time;
    if due_time > 0 {
        due_time = due_time.neg();
    }

    record.timer.init();
    record.dpc.init(timer_dpc, Some(data.clone()));

    extension.queue_lock.acquire();

    if irp.is_cancel() {
        if irp.set_cancel_rtn(None) != None {
            extension.queue_lock.release();
            return Err(EventError::Status(STATUS_CANCELLED));
        }
    }

    irp.mark_pending();

    record.timer.set(due_time, Some(&record.dpc));

    extension.event_queue.push(record);

    extension.queue_lock.release();

    Ok(STATUS_PENDING)
}

fn event_notification(
    device: &WduDevice,
    irp: &mut WduIrp,
    register_event: &RegisterEvent,
) -> EventResult<NTSTATUS> {
    info!("\ttRegisterEventBasedNotification");
    let extension: &mut DeviceExtension = device.extension_as_mut_ref();

    let access_mask = 0x00100000 | 0x0002;

    let event = WduEvent::ref_by_handle(register_event.event, access_mask, irp.processor_mode())?;

    let data = NotifyData{
        ty: NotifyType::EventBased(event),
        extension: device.extension_as_mut_ptr(),
    };

    let mut record = Box::new(NotifyRecord {
        file_object: irp.file_object(),
        timer: WduTimer::default(),
        dpc: WduDpc::new(),
        data: data.clone()
    });


    let mut due_time = register_event.due_time;
    if due_time > 0 {
        due_time = due_time.neg();
    }

    record.timer.init();
    record.dpc.init(timer_dpc, Some(data.clone()));

    extension.queue_lock.acquire();

    record.timer.set(due_time, Some(&record.dpc));
    extension.event_queue.push(record);

    extension.queue_lock.release();

    Ok(STATUS_SUCCESS)
}

fn cancel_routine(device: &WduDevice, irp: &mut WduIrp) {
    info!("==>EventCancelRoutine irp {:?}", irp.as_ptr());
    let dev_ext: &mut DeviceExtension = device.extension_as_mut_ref();
    irp.release_cancel_lock();

    dev_ext.queue_lock.acquire();
    let index = dev_ext
        .event_queue
        .iter()
        .position(|x| {
            match &x.data.ty {
                NotifyType::IrpBased(i) => i.as_ptr() == irp.as_ptr(),
                _ => false,
            }
        })
        .unwrap();

    let mut pending_irp = dev_ext.event_queue.swap_remove(index);

    if pending_irp.timer.cancel() {
        info!("\tCancelled timer");
    } // TODO: check this!

    dev_ext.queue_lock.release();

    info!("\tCancelled IRP {:?}", irp.as_ptr());
    let io_status = WduIoStatus::new_with_status(STATUS_CANCELLED);
    irp.complete(io_status);
}

fn timer_dpc(dpc: &mut EventDpc) {
    info!("==> CustomTimerDPC");
    // Context can't be null!!
    let mut remove = true;
    let data = dpc.context_as_mut_ref().unwrap();

    if data.extension.is_null() {
        debug_assert!(false, "Null DeviceExtension in DPC");
        return;
    }

    // Let's derefence to avoid having to derefence on each call
    let extension = unsafe { &mut (*data.extension) };

    extension.queue_lock.acquire_at_dpc();

    let index = extension
        .event_queue
        .iter()
        .position(|x| &x.data == data );

    // Entry was already removed probably by the cleanup function so let's release the lock and
    // return
    if index.is_none() {
        extension.queue_lock.release_from_dpc();
        return;
    }

    match &mut data.ty {
        NotifyType::IrpBased(ref mut irp) => {
            if irp.set_cancel_rtn(None).is_some() {
                extension.queue_lock.release_from_dpc();
                let io_status = WduIoStatus::success_no_info();
                irp.complete(io_status);
                extension.queue_lock.acquire_at_dpc();
            } else {
                //
                // Cancel routine will run as soon as we release the lock.
                // So let's not remove the record from the extension so the cancel routine
                // can complete the request and free the record.
                //
                remove = false;
            }
        }
        NotifyType::EventBased(ref mut evt) => {
            evt.set(0, false);
            evt.dereference();
        }
    }

    if remove {
        let _ = extension.event_queue.swap_remove(index.unwrap());
    }

    extension.queue_lock.release_from_dpc();
}

use crate::io::{
    create::WduCreate,
    device::WduDevice,
    device_control::WduDeviceControl,
    file_obj::WduFileObject,
    irp::{MajorFunction, WduIrp, WduPnpIrp, WduPowerIrp},
};
use core::ffi::c_void;
use snafu::Snafu;
use windows_sys::{
    Wdk::{
        Foundation::{DEVICE_OBJECT, DRIVER_OBJECT, IRP},
        System::SystemServices::{
            IoAllocateDriverObjectExtension, IoGetDriverObjectExtension, IRP_MJ_CLEANUP,
            IRP_MJ_CLOSE, IRP_MJ_CREATE, IRP_MJ_DEVICE_CONTROL, IRP_MJ_READ, IRP_MJ_WRITE,
        },
    },
    Win32::Foundation::{NTSTATUS, STATUS_SUCCESS},
};

#[derive(Debug, Snafu)]
pub enum WduDriverError {
    #[snafu(display("Unable to allocate Driver Extension"))]
    DriverExtAllocFailed,
    #[snafu(display("WduDriver already initialized"))]
    AlreadyInit,
}

pub type WduDriverResult<T> = Result<T, WduDriverError>;

pub struct WduDriver {
    init: bool,
    driver: *mut DRIVER_OBJECT,
    // registry_path: WduUnicodeString, TODO
    add_device: Option<WduAddDevice>,
    unload: Option<WduDriverUnload>,
    io: IoDispath,
    fileobj: FileObjDispatch,
}

pub type WduAddDevice = fn(&WduDriver) -> NTSTATUS;
pub type WduDriverUnload = fn(&WduDriver) -> ();

// FileObject dispatch function definitions
pub type WduCreateDispatch = fn(&WduDevice, &mut WduIrp, WduCreate);
pub type WduCloseCleanupDispatch = fn(&WduDevice, &mut WduIrp, WduFileObject);

// I/O dispatch function definitions (TODO: Check if we really need the NTSTATUS)
pub type WduReadWriteDispatch = fn(&WduDevice, &mut WduIrp, usize) -> NTSTATUS;
pub type WduIoctlDispatch = fn(&WduDevice, &mut WduIrp, WduDeviceControl) -> NTSTATUS;

// Power Dispatch function definitions
pub type WduPowerDispatch = fn(&WduDevice, &mut WduIrp, WduPowerIrp) -> NTSTATUS;

// PNP Dispatch function definitions
pub type WduPnpDispatch = fn(&WduDevice, &mut WduIrp, WduPnpIrp) -> NTSTATUS;

const WKR_DRIVER_ID: *mut c_void = WduDriver::get_wdu_driver as *mut c_void;

#[derive(Default)]
pub struct FileObjDispatch {
    create: Option<WduCreateDispatch>,
    close: Option<WduCloseCleanupDispatch>,
    cleanup: Option<WduCloseCleanupDispatch>,
}

impl FileObjDispatch {
    pub fn create_irp(mut self, create: WduCreateDispatch) -> Self {
        self.create = Some(create);
        self
    }

    pub fn close_irp(mut self, close: WduCloseCleanupDispatch) -> Self {
        self.close = Some(close);
        self
    }

    pub fn cleanup_irp(mut self, cleanup: WduCloseCleanupDispatch) -> Self {
        self.cleanup = Some(cleanup);
        self
    }
}

#[derive(Default)]
pub struct IoDispath {
    read: Option<WduReadWriteDispatch>,
    write: Option<WduReadWriteDispatch>,
    ioctl: Option<WduIoctlDispatch>,
}

impl IoDispath {
    pub fn read_irp(mut self, read: WduReadWriteDispatch) -> Self {
        self.read = Some(read);
        self
    }

    pub fn write_irp(mut self, write: WduReadWriteDispatch) -> Self {
        self.write = Some(write);
        self
    }

    pub fn ioctl_irp(mut self, ioct: WduIoctlDispatch) -> Self {
        self.ioctl = Some(ioct);
        self
    }
}

impl WduDriver {
    pub fn new(driver: *mut DRIVER_OBJECT) -> Self {
        Self {
            driver,
            add_device: None,
            unload: None,
            init: false,
            io: IoDispath::default(),
            fileobj: FileObjDispatch::default(),
        }
    }

    pub(crate) fn as_ptr(&self) -> *const DRIVER_OBJECT {
        self.driver as *const _
    }

    pub fn device(&self) -> WduDevice {
        // If no DeviceObject this is equivalent to doing WduDevice::default()
        let dev_obj = unsafe { (*self.driver).DeviceObject };
        WduDevice::wrap_device(dev_obj)
    }

    pub fn device_add(mut self, device_add: WduAddDevice) -> Self {
        unsafe {
            let ext = (*self.driver).DriverExtension;
            if !ext.is_null() {
                let pfn = Self::add_device as *mut u8;
                (*ext).AddDevice = Some(core::mem::transmute_copy(&pfn));
            }
        }

        self.add_device = Some(device_add);
        self
    }

    pub fn unload(mut self, driver_unload: WduDriverUnload) -> Self {
        unsafe {
            let pfn = Self::driver_unload as *mut u8;
            (*self.driver).DriverUnload = Some(core::mem::transmute_copy(&pfn));
        }

        self.unload = Some(driver_unload);
        self
    }

    pub fn file_object(mut self, dispatch_rtns: FileObjDispatch) -> Self {
        let file_obj_mj = [IRP_MJ_CREATE, IRP_MJ_CLOSE, IRP_MJ_CLEANUP];

        let pfn = Self::dispatch_handler as *mut u8;
        for major in file_obj_mj {
            unsafe {
                (*self.driver).MajorFunction[major as usize] =
                    Some(core::mem::transmute_copy(&pfn));
            }
        }

        self.fileobj = dispatch_rtns;
        self
    }

    pub fn io(mut self, dispatch_rtns: IoDispath) -> Self {
        let io_mj = [IRP_MJ_READ, IRP_MJ_WRITE, IRP_MJ_DEVICE_CONTROL];

        let pfn = Self::dispatch_handler as *mut u8;
        for major in io_mj {
            unsafe {
                (*self.driver).MajorFunction[major as usize] = Some(core::mem::transmute_copy(&pfn))
            }
        }

        self.io = dispatch_rtns;
        self
    }

    pub fn power(mut self) -> Self {
        todo!()
    }

    pub fn pnp(mut self) -> Self {
        todo!()
    }

    pub fn build(mut self) -> WduDriverResult<Self> {
        if self.init {
            return Err(WduDriverError::AlreadyInit);
        }

        unsafe {
            self.alloc_ext()?;
        }

        Ok(self)
    }

    // TODO: Pass WduDevice, check if filter, etc...
    fn add_device_internal(&self, device: &WduDevice) -> NTSTATUS {
        self.add_device
            .map_or_else(|| STATUS_SUCCESS, |add_device| add_device(self))
    }

    fn unload_internal(&self) {
        self.unload.map(|unload| unload(self));
    }

    fn fo_dispatch(&self, device: &WduDevice, irp: &mut WduIrp) {
        match irp.major() {
            MajorFunction::Create => self.fileobj.create.map(|pfn| {
                let create = unsafe { WduCreate::new(irp) };
                pfn(device, irp, create)
            }),
            MajorFunction::Close => self.fileobj.close.map(|pfn| {
                let file_object = irp.file_object();
                pfn(device, irp, file_object)
            }),
            MajorFunction::Cleanup => self.fileobj.cleanup.map(|pfn| {
                let file_object = irp.file_object();
                pfn(device, irp, file_object)
            }),
            _ => unreachable!("Invalid FileObject dispatch"),
        };
    }

    fn io_dispatch(&self, device: &WduDevice, irp: &mut WduIrp) -> NTSTATUS {
        let status = match irp.major() {
            MajorFunction::Read => self.io.read.map(|pfn| pfn(device, irp, 0)),
            MajorFunction::Write => self.io.write.map(|pfn| pfn(device, irp, 0)),
            MajorFunction::DeviceControl => self.io.ioctl.map(|pfn| {
                let ioctl = unsafe { WduDeviceControl::new(irp) };
                pfn(device, irp, ioctl)
            }),
            _ => unreachable!("Invalid I/O dispatch"),
        };

        status.map_or_else(|| STATUS_SUCCESS, |status| status)
    }

    fn power_dispatch(&self, device: &WduDevice, irp: &mut WduIrp) -> NTSTATUS {
        todo!()
    }

    fn pnp_dispatch(&self) -> NTSTATUS {
        todo!()
    }

    #[no_mangle]
    unsafe extern "system" fn driver_unload(driver: *const DRIVER_OBJECT) {
        let wdu_driver = Self::get_wdu_driver(driver);
        if wdu_driver.is_null() {
            return;
        }

        assert!((*wdu_driver).init);
        (*wdu_driver).unload_internal();
    }

    #[no_mangle]
    unsafe extern "system" fn add_device(
        driver: *const DRIVER_OBJECT,
        pdo: *const DEVICE_OBJECT,
    ) -> NTSTATUS {
        let wdu_driver = Self::get_wdu_driver(driver);

        // TODO: Check what's the best option in this case!
        if wdu_driver.is_null() {
            return STATUS_SUCCESS;
        }

        // Wrap PDO in WduDevice and pass it to internal function
        let wdu_device = WduDevice::wrap_device(pdo);
        (*wdu_driver).add_device_internal(&wdu_device)
    }

    #[no_mangle]
    unsafe extern "system" fn dispatch_handler(
        device: *const DEVICE_OBJECT,
        irp: *const IRP,
    ) -> NTSTATUS {
        let mut status = STATUS_SUCCESS;

        let mut wdu_irp = WduIrp::wrap(irp);

        // Wrap DO into WduDevice and retrieve WduDevice from DeviceExtension
        let wdu_device = WduDevice::get_wdu_device(device);

        // Get DRIVER_OBJECT from DO to obtain WduDriver from DriverObject Extension
        let wdu_driver = WduDriver::get_wdu_driver(wdu_device.get_driver());

        if wdu_driver.is_null() {
            // TODO: log error but don't fail operation, if we are exeucting this handler it
            //  should never be the case
            return status;
        }

        if wdu_irp.is_fileobj() {
            (*wdu_driver).fo_dispatch(&wdu_device, &mut wdu_irp);
        } else if wdu_irp.is_io() {
            status = (*wdu_driver).io_dispatch(&wdu_device, &mut wdu_irp);
        } else if wdu_irp.is_power() {
            status = (*wdu_driver).power_dispatch(&wdu_device, &mut wdu_irp);
        } else if wdu_irp.is_pnp() {
            status = (*wdu_driver).pnp_dispatch();
        } else {
            unreachable!("Unknown operation. Device: {:?} - IRP: {:?}", device, irp)
        };

        status
    }

    unsafe fn alloc_ext(&mut self) -> WduDriverResult<()> {
        let mut ext: *mut c_void = core::ptr::null_mut();
        let status = IoAllocateDriverObjectExtension(
            self.as_ptr(),
            WKR_DRIVER_ID,
            core::mem::size_of::<WduDriver>() as u32,
            &mut ext as *mut _ as *mut _,
        );

        if status != STATUS_SUCCESS {
            return Err(WduDriverError::DriverExtAllocFailed);
        }

        self.init = true;

        unsafe {
            // Copy WduDriver to the allocated extension. This allow us to safely drop the
            // WduDriver after DriverEntry while keeping a copy stored in the DRIVER_OBJECT
            // extension. This will be freed by the OS when the Driver unloads.
            ext.copy_from(
                self as *const WduDriver as *const _,
                core::mem::size_of::<WduDriver>(),
            );
        }

        Ok(())
    }

    fn get_wdu_driver(driver: *const DRIVER_OBJECT) -> *const WduDriver {
        unsafe { IoGetDriverObjectExtension(driver, WKR_DRIVER_ID) as *const _ }
    }
}

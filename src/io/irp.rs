use crate::{
    inner_getters_ptr,
    io::{device::WduDevice, file_obj::WduFileObject},
    memory::mdl::WduMdl,
    ProcessorMode,
};
use alloc::boxed::Box;
use core::ffi::c_void;
use windows_sys::{
    Wdk::{
        Foundation::{DEVICE_OBJECT, IO_STACK_LOCATION, IRP},
        System::SystemServices::{
            IoForwardIrpSynchronously, IoReleaseCancelSpinLock, IofCompleteRequest, IRP_MJ_CLEANUP,
            IRP_MJ_CLOSE, IRP_MJ_CREATE, IRP_MJ_DEVICE_CONTROL, IRP_MJ_READ, IRP_MJ_WRITE,
            SL_PENDING_RETURNED,
        },
    },
    Win32::Foundation::{NTSTATUS, STATUS_SUCCESS, STATUS_UNSUCCESSFUL},
};

pub type WduCancelRoutine = fn(&WduDevice, &mut WduIrp) -> ();

#[derive(Clone)]
pub struct WduIrp {
    irp: *mut IRP,
    mj: MajorFunction,
    cancel: Option<WduCancelRoutine>,
}

inner_getters_ptr!(WduIrp, irp, IRP);

#[derive(Copy, Clone)]
pub(crate) enum MajorFunction {
    Create,
    Close,
    Cleanup,
    Read,
    Write,
    DeviceControl,
}

pub enum WduPowerIrp {}
pub enum WduPnpIrp {}

#[derive(Clone)]
pub struct WduIoStatus {
    status: NTSTATUS,
    info: usize,
}

impl From<u32> for MajorFunction {
    fn from(value: u32) -> Self {
        match value {
            IRP_MJ_CREATE => MajorFunction::Create,
            IRP_MJ_CLOSE => MajorFunction::Close,
            IRP_MJ_CLEANUP => MajorFunction::Cleanup,
            IRP_MJ_READ => MajorFunction::Read,
            IRP_MJ_WRITE => MajorFunction::Write,
            IRP_MJ_DEVICE_CONTROL => MajorFunction::DeviceControl,
            _ => todo!(),
        }
    }
}

impl MajorFunction {
    fn is_io(&self) -> bool {
        match self {
            MajorFunction::Write | MajorFunction::Read | MajorFunction::DeviceControl => true,
            _ => false,
        }
    }

    fn is_fileobj(&self) -> bool {
        match self {
            MajorFunction::Create | MajorFunction::Close | MajorFunction::Cleanup => true,
            _ => false,
        }
    }

    fn is_power(&self) -> bool {
        todo!()
    }

    fn is_pnp(&self) -> bool {
        todo!()
    }
}

impl WduIoStatus {
    pub fn new() -> Self {
        WduIoStatus {
            status: STATUS_UNSUCCESSFUL,
            info: 0,
        }
    }

    pub fn new_with_status(status: NTSTATUS) -> Self {
        WduIoStatus {
            status: status,
            info: 0,
        }
    }

    pub fn success_no_info() -> Self {
        WduIoStatus {
            status: STATUS_SUCCESS,
            info: 0,
        }
    }

    pub fn success_with_info(info: usize) -> Self {
        WduIoStatus {
            status: STATUS_SUCCESS,
            info,
        }
    }

    pub fn set_status(&mut self, status: NTSTATUS) {
        self.status = status;
    }

    pub fn status(&self) -> NTSTATUS {
        self.status
    }

    pub fn set_info(&mut self, info: usize) {
        self.info = info;
    }
}

// TODO: SetcompletionRoutine, CopyCurrentIrpStackToNext & IoCallDriver
impl WduIrp {
    pub(crate) fn wrap(irp: *const IRP) -> Self {
        let mj = MajorFunction::from(unsafe { (*Self::current_stack(irp)).MajorFunction as u32 });
        WduIrp {
            irp: irp as *mut _,
            mj,
            cancel: None,
        }
    }

    pub(crate) fn is_io(&self) -> bool {
        self.mj.is_io()
    }

    pub(crate) fn is_fileobj(&self) -> bool {
        self.mj.is_fileobj()
    }

    pub(crate) fn is_power(&self) -> bool {
        self.mj.is_power()
    }

    pub(crate) fn is_pnp(&self) -> bool {
        self.mj.is_pnp()
    }

    pub(crate) fn major(&self) -> MajorFunction {
        self.mj
    }

    fn complete_request(&self, boost: i8) {
        unsafe {
            IofCompleteRequest(self.irp as *const _, boost);
        }
    }

    pub fn forward_sync(&self, device: &WduDevice) -> bool {
        let res = unsafe { IoForwardIrpSynchronously(device.device(), self.irp) };

        u8::from(res) == 1
    }

    // TODO: Consider if we should consume self
    pub fn complete(&mut self, io_status: WduIoStatus) {
        if self.irp.is_null() {
            return;
        }

        unsafe {
            (*self.irp).IoStatus.Anonymous.Status = io_status.status;
            (*self.irp).IoStatus.Information = io_status.info;
        }

        self.complete_request(0);
    }

    /*
    NT_ASSERT(Irp->CurrentLocation <= Irp->StackCount + 1);
    return Irp->Tail.Overlay.CurrentStackLocation;
    */
    // TODO: Wrap IO_STACK_LOCATION
    pub(crate) fn current_stack(irp: *const IRP) -> *const IO_STACK_LOCATION {
        unsafe {
            assert!((*irp).CurrentLocation <= (*irp).StackCount + 1);
            (*irp)
                .Tail
                .Overlay
                .Anonymous2
                .Anonymous
                .CurrentStackLocation as *const _
        }
    }

    pub(crate) fn current_stack_as_mut(irp: *const IRP) -> *mut IO_STACK_LOCATION {
        unsafe {
            assert!((*irp).CurrentLocation <= (*irp).StackCount + 1);
            (*irp)
                .Tail
                .Overlay
                .Anonymous2
                .Anonymous
                .CurrentStackLocation as *mut _
        }
    }

    pub(crate) unsafe fn system_buffer(&self) -> *mut c_void {
        (*self.irp).AssociatedIrp.SystemBuffer
    }

    pub(crate) unsafe fn user_buffer(&self) -> *mut c_void {
        (*self.irp).UserBuffer
    }

    pub(crate) unsafe fn mdl_address(&self) -> WduMdl {
        WduMdl::wrap((*self.irp).MdlAddress)
    }

    pub fn file_object(&self) -> WduFileObject {
        unsafe { WduFileObject::wrap((*Self::current_stack(self.irp)).FileObject) }
    }

    pub fn original_file_object(&self) -> WduFileObject {
        unsafe { WduFileObject::wrap((*self.irp).Tail.Overlay.OriginalFileObject) }
    }

    pub fn is_cancel(&self) -> bool {
        unsafe { (*self.irp).Cancel == u8::from(true) }
    }

    pub fn mark_pending(&mut self) {
        unsafe {
            (*Self::current_stack_as_mut(self.irp)).Control |= SL_PENDING_RETURNED as u8;
        }
    }

    pub fn release_cancel_lock(&self) {
        unsafe {
            IoReleaseCancelSpinLock((*self.irp).CancelIrql);
        }
    }

    pub fn processor_mode(&self) -> ProcessorMode {
        ProcessorMode::from(unsafe { (*self.irp).RequestorMode })
    }

    // TODO: Add proper error handling in below functions
    fn set_context(&mut self) {
        unsafe {
            (*self.irp).Tail.Overlay.Anonymous1.Anonymous.DriverContext[3] =
                Box::into_raw(Box::new(self.clone())) as _;
        }
    }

    fn clear_context(&mut self) {
        unsafe {
            (*self.irp).Tail.Overlay.Anonymous1.Anonymous.DriverContext[3] = core::ptr::null_mut()
        }
    }

    fn from_context(irp: *const IRP) -> Box<Self> {
        unsafe {
            Box::from_raw((*irp).Tail.Overlay.Anonymous1.Anonymous.DriverContext[3] as *mut _)
        }
    }

    // TODO: I don't like this very much, we should make this atomic.
    //  MS implementation "uses an interlocked exchange intrinsic to set the address of the
    //  Cancel routine as an atomic operation"
    //  Consider how WDF is doing this with MarkCancelable(Ex)/UnmarkCancelable
    pub fn set_cancel_rtn(&mut self, cancel: Option<WduCancelRoutine>) -> Option<WduCancelRoutine> {
        let prev = self.cancel;
        self.cancel = cancel;

        unsafe {
            if cancel.is_some() {
                self.set_context();

                let pfn = Self::cancel_routine as *mut u8;
                (*self.irp).CancelRoutine = Some(core::mem::transmute_copy(&pfn));
            } else {
                (*self.irp).CancelRoutine = None;

                let _ = Self::from_context(self.irp); // Get context to deallocate
                self.clear_context(); // Set context to null
            }
        }
        prev
    }

    #[no_mangle]
    unsafe extern "system" fn cancel_routine(device: *mut DEVICE_OBJECT, irp: *mut IRP) {
        let device = WduDevice::wrap_device(device);
        let mut original_irp = WduIrp::wrap(irp);

        let wdu_irp = WduIrp::from_context(irp);

        wdu_irp
            .cancel
            .map_or_else(|| (), |pfn| pfn(&device, &mut original_irp))
    }
}

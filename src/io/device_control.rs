use crate::{
    io::irp::WduIrp,
    memory::mdl::{PagePriority, WduMdl},
};
use core::ffi::c_void;
use windows_sys::Win32::System::Ioctl::{
    METHOD_BUFFERED, METHOD_IN_DIRECT, METHOD_NEITHER, METHOD_OUT_DIRECT,
};

/// Macro to define a I/O Control code (CTL_CODE)
#[macro_export]
macro_rules! encode_ioctl {
    ($device_type:expr, $function_code:expr, $method:expr, $access:expr) => {
        (($device_type << 16) | ($access << 14) | ($function_code << 2) | $method)
    };
}

macro_rules! method_from_ioctl {
    ($ioctl:expr) => {
        $ioctl & 0x3
    };
}

/*
 Hack until wdkmetadata has the right layout of IO_STACK_LOCATION.Parameters.DeviceIoControl.
 windows_sys defines DeviceIoControl Out/In buffer and Ioctl as u32, in x64 these values are
 defined as unsigned long.
 This hack fixes this mismatch in the layout
*/
#[repr(C)]
#[allow(non_snake_case)]
struct DeviceIoControlRaw {
    OutputBufferLength: usize,
    InputBufferLength: usize,
    IoControlCode: usize,
    Type3InputBuffer: *mut core::ffi::c_void,
}

pub enum WduIocltBuffers {
    Unknown,
    Buffered(*mut c_void),
    Direct((*mut c_void, WduMdl)),
    Neither((*mut c_void, *mut c_void)),
}

pub struct WduDeviceControl {
    ioctl: u32,
    out_buf_len: usize,
    in_buf_len: usize,
    buffer: WduIocltBuffers,
}

impl WduDeviceControl {
    pub(crate) unsafe fn new(irp: &WduIrp) -> Self {
        let current_stack = WduIrp::current_stack(irp.as_ptr());
        let parameters = (*current_stack).Parameters;

        // Temporary
        let data: DeviceIoControlRaw = core::mem::transmute(parameters);

        let ioctl = data.IoControlCode as u32;
        let in_buf_len = data.InputBufferLength;
        let out_buf_len = data.OutputBufferLength;

        let method_bits = method_from_ioctl!(ioctl);

        let buffer = match method_bits {
            METHOD_BUFFERED => WduIocltBuffers::Buffered(irp.system_buffer()),
            METHOD_IN_DIRECT | METHOD_OUT_DIRECT => {
                let buffers = (irp.system_buffer(), irp.mdl_address());
                WduIocltBuffers::Direct(buffers)
            }
            METHOD_NEITHER => {
                let buffers = (data.Type3InputBuffer, irp.user_buffer());
                WduIocltBuffers::Neither(buffers)
            }
            _ => WduIocltBuffers::Unknown,
        };

        Self {
            ioctl,
            in_buf_len,
            out_buf_len,
            buffer,
        }
    }

    pub fn ioctl(&self) -> u32 {
        self.ioctl
    }

    pub fn input_buffer_size(&self) -> usize {
        self.in_buf_len
    }

    pub fn output_buffer_size(&self) -> usize {
        self.out_buf_len
    }

    // TODO: Consider returning this as Option<T> or T
    pub fn input_buffer(&self) -> *const c_void {
        let buffer = match self.buffer {
            WduIocltBuffers::Buffered(buffer) => buffer,
            WduIocltBuffers::Neither((input, _)) => input,
            WduIocltBuffers::Direct((input, _)) => input,
            WduIocltBuffers::Unknown => core::ptr::null_mut(),
        };
        buffer as *const _
    }

    // TODO: Consider returning this as a Vec<u8>
    pub fn output_buffer(&self) -> *mut c_void {
        match &self.buffer {
            WduIocltBuffers::Buffered(buffer) => buffer.clone(),
            WduIocltBuffers::Neither((_, output)) => output.clone(),
            WduIocltBuffers::Direct((_, mdl)) => {
                mdl.get_system_addr(PagePriority::Normal | PagePriority::MdlMappingNoExecute)
            }
            WduIocltBuffers::Unknown => core::ptr::null_mut(),
        }
    }

    // Direct I/O specific method
    pub fn mdl_byte_count(&self) -> usize {
        match &self.buffer {
            WduIocltBuffers::Direct((_, mdl)) => mdl.byte_count() as usize,
            _ => 0,
        }
    }
}

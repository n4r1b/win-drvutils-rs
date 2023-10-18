use crate::nt::ExEventObjectType;
use crate::{
    dereference, inner_getters_ptr, inner_getters_value, ref_by_handle, ProcessorMode, WduResult,
};
use windows_sys::Wdk::Foundation::POBJECT_TYPE;
use windows_sys::Win32::Foundation::{HANDLE, STATUS_SUCCESS};
use windows_sys::{
    Wdk::{
        Foundation::KEVENT,
        System::SystemServices::{
            KeClearEvent, KeInitializeEvent, KePulseEvent, KeReadStateEvent, KeResetEvent,
            KeSetEvent,
        },
    },
    Win32::System::Kernel::EVENT_TYPE,
};

#[cfg(feature = "const_new")]
use const_zero::const_zero;

pub enum WduEventType {
    NotificationEvent,
    SynchronizationEvent,
}

impl Into<EVENT_TYPE> for WduEventType {
    fn into(self) -> EVENT_TYPE {
        match self {
            WduEventType::NotificationEvent => 0,
            WduEventType::SynchronizationEvent => 1,
        }
    }
}

#[derive(Clone)]
pub struct WduEvent {
    event: *mut KEVENT,
}

impl Default for WduEvent {
    fn default() -> Self {
        Self::new()
    }
}

inner_getters_ptr!(WduEvent, event, KEVENT);

// TODO: Consider ZwEvent related functions
impl WduEvent {
    #[cfg(feature = "const_new")]
    pub const fn const_new() -> Self {
        WduEvent {
            event: core::ptr::null_mut(),
        }
    }

    pub fn new() -> Self {
        WduEvent {
            event: core::ptr::null_mut(),
        }
    }

    pub fn init(&mut self, event_type: WduEventType, state: bool) {
        unsafe {
            KeInitializeEvent(self.as_mut_ptr(), event_type.into(), u8::from(state));
        }
    }

    pub fn reset(&mut self) -> i32 {
        unsafe { KeResetEvent(self.as_mut_ptr()) }
    }

    pub fn clear(&mut self) {
        unsafe {
            KeClearEvent(self.as_mut_ptr());
        }
    }

    pub fn set(&mut self, increment: i32, wait: bool) -> i32 {
        unsafe { KeSetEvent(self.as_mut_ptr(), increment, u8::from(wait)) }
    }

    pub fn read_state(&mut self) -> i32 {
        unsafe { KeReadStateEvent(self.as_mut_ptr()) }
    }

    pub fn pulse(&mut self, increment: i32, wait: bool) -> i32 {
        unsafe { KePulseEvent(self.as_mut_ptr(), increment, u8::from(wait)) }
    }

    pub fn ref_by_handle(
        handle: HANDLE,
        access_mask: u32,
        access_mode: ProcessorMode,
    ) -> WduResult<Self> {
        let mut object = WduEvent::new();

        ref_by_handle(
            handle,
            access_mask,
            unsafe { Some(*ExEventObjectType) },
            access_mode,
            &mut object.event,
        )?;

        Ok(object)
    }

    pub fn dereference(&self) {
        dereference(self.as_ptr() as *const _);
    }
}

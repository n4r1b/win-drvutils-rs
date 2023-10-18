use crate::common::dpc::WduDpc;
use crate::inner_getters_value;
use windows_sys::{
    Wdk::{
        Foundation::{PEX_TIMER, PIO_TIMER},
        System::SystemServices::{
            ExCancelTimer, ExDeleteTimer, ExSetTimer, KeCancelTimer, KeInitializeTimer,
            KeInitializeTimerEx, KeReadStateTimer, KeSetCoalescableTimer, KeSetTimer, KeSetTimerEx,
            EX_TIMER_HIGH_RESOLUTION, EX_TIMER_NO_WAKE, KTIMER,
        },
    },
    Win32::System::Kernel::{NotificationTimer, SynchronizationTimer, TIMER_TYPE},
};

pub enum WduTimerType {
    NotificationTimer,
    SynchronizationTimer,
}

impl Into<TIMER_TYPE> for WduTimerType {
    fn into(self) -> TIMER_TYPE {
        match self {
            WduTimerType::NotificationTimer => NotificationTimer,
            WduTimerType::SynchronizationTimer => SynchronizationTimer,
        }
    }
}

pub enum WduExTimerAttributes {
    HighResolution,
    NoWake,
    Notification,
    UnlimitedTolerance,
}

impl Into<u32> for WduExTimerAttributes {
    fn into(self) -> u32 {
        match self {
            WduExTimerAttributes::HighResolution => EX_TIMER_HIGH_RESOLUTION,
            WduExTimerAttributes::NoWake => EX_TIMER_NO_WAKE,
            // #define EX_TIMER_NOTIFICATION (1UL << 31)
            WduExTimerAttributes::Notification => 1 << 31,
            // #define EX_TIMER_UNLIMITED_TOLERANCE ((LONGLONG)-1)
            WduExTimerAttributes::UnlimitedTolerance => u32::MAX,
        }
    }
}

// TODO: Consider implementing Drop to make sure we cancel the Timer when dropping
pub struct WduTimer {
    timer: KTIMER,
}

pub struct WduIoTimer {
    timer: PIO_TIMER,
}

pub struct WduExTimer {
    timer: PEX_TIMER,
}

impl Default for WduTimer {
    fn default() -> Self {
        Self::new()
    }
}

inner_getters_value!(WduTimer, timer, KTIMER);
inner_getters_value!(WduExTimer, timer, PEX_TIMER);

impl WduTimer {
    pub fn new() -> Self {
        Self {
            timer: unsafe { core::mem::zeroed() },
        }
    }

    pub fn init(&mut self) {
        unsafe {
            KeInitializeTimer(self.as_mut_ptr());
        }
    }

    pub fn init_ex(timer_type: WduTimerType) -> Self {
        let mut timer = Self {
            timer: unsafe { core::mem::zeroed() },
        };

        unsafe {
            KeInitializeTimerEx(timer.as_mut_ptr(), timer_type.into());
        }
        timer
    }

    pub fn cancel(&mut self) -> bool {
        unsafe { KeCancelTimer(self.as_mut_ptr()) == u8::from(true) }
    }

    pub fn read_state(&mut self) -> bool {
        unsafe { KeReadStateTimer(self.as_mut_ptr()) == u8::from(true) }
    }

    pub fn set_coalescable<T, U, V>(
        &mut self,
        duetime: i64,
        period: u32,
        tolerable_delay: u32,
        dpc: Option<&WduDpc<T, U, V>>,
    ) -> bool {
        unsafe {
            KeSetCoalescableTimer(
                self.as_mut_ptr(),
                duetime,
                period,
                tolerable_delay,
                dpc.map_or_else(|| core::ptr::null(), |dpc| dpc.as_ptr()),
            ) == u8::from(true)
        }
    }

    pub fn set<T, U, V>(&mut self, duetime: i64, dpc: Option<&WduDpc<T, U, V>>) -> bool {
        unsafe {
            KeSetTimer(
                self.as_mut_ptr(),
                duetime,
                dpc.map_or_else(|| core::ptr::null(), |dpc| dpc.as_ptr()),
            ) == u8::from(true)
        }
    }

    // TODO: Add WduDpc
    pub fn set_ex(&mut self, duetime: i64, period: i32) -> bool {
        unsafe {
            KeSetTimerEx(self.as_mut_ptr(), duetime, period, core::ptr::null_mut())
                == u8::from(true)
        }
    }
}

impl WduExTimer {
    // Has to return Option, if ExAllocateTimer fails will return nullptr then return none
    pub fn init() -> Self {
        let timer = Self {
            timer: unsafe { core::mem::zeroed() },
        };

        timer
    }

    // TODO: Create WduTimerParams(Delete)
    pub fn delete(&mut self, cancel: bool, wait: bool) -> bool {
        unsafe {
            ExDeleteTimer(
                self.get(),
                u8::from(cancel),
                u8::from(wait),
                core::ptr::null_mut(),
            ) == u8::from(true)
        }
    }

    // TODO: Create WduTimerParams(Set)
    pub fn set(&mut self, duetime: i64, period: i64) -> bool {
        unsafe { ExSetTimer(self.get(), duetime, period, core::ptr::null_mut()) == u8::from(true) }
    }

    // TODO: Create WduTimerParams(Cancel)
    pub fn cancel(&mut self) -> bool {
        unsafe { ExCancelTimer(self.get(), core::ptr::null_mut()) == u8::from(true) }
    }
}

// TODO: Create pub function for ExSetTimerResolution & ExQueryTimerResolution

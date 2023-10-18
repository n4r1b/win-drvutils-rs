use core::ffi::c_void;
use windows_sys::Wdk::{
    Foundation::KDPC,
    System::SystemServices::{KeInitializeDpc, KeInsertQueueDpc},
};

pub type WduDeferredRtn<T, U, V> = fn(&mut WduDpc<T, U, V>) -> ();

pub struct WduDpc<T, U, V> {
    dpc: KDPC,
    context: Option<T>,
    routine: Option<WduDeferredRtn<T, U, V>>,
}

pub struct WduThreadedDpc {
    dpc: KDPC,
}

impl<T, U, V> WduDpc<T, U, V> {
    pub fn new() -> Self {
        Self {
            dpc: unsafe { core::mem::zeroed() },
            context: None,
            routine: None,
        }
    }

    #[inline(always)]
    pub fn context_as_ref(&self) -> Option<&T> {
        self.context.as_ref()
    }

    #[inline(always)]
    pub fn context_as_mut_ref(&mut self) -> Option<&mut T> {
        self.context.as_mut()
    }

    /*
    TODO: Get values from KPDC
    #[inline(always)]
    pub fn arg1(&self) -> Option<&U> {
        self.arg1.as_ref()
    }

    #[inline(always)]
    pub fn arg1_mut(&mut self) -> Option<&mut U> {
        self.arg1.as_mut()
    }

    #[inline(always)]
    pub fn arg2(&self) -> Option<&V> {
        self.arg2.as_ref()
    }

    #[inline(always)]
    pub fn arg2_mut(&mut self) -> Option<&mut V> {
        self.arg2.as_mut()
    }
     */

    pub fn as_ref(&self) -> &KDPC {
        &self.dpc
    }

    pub fn as_ptr(&self) -> *const KDPC {
        &self.dpc as *const _
    }

    pub fn as_mut_ptr(&mut self) -> *mut KDPC {
        &self.dpc as *const _ as *mut _
    }

    pub fn init(&mut self, routine: WduDeferredRtn<T, U, V>, context: Option<T>) {
        self.routine = Some(routine);
        self.context = context;

        unsafe {
            let pfn = Self::custom_dpc as *mut u8;

            KeInitializeDpc(
                self.as_mut_ptr(),
                Some(core::mem::transmute_copy(&pfn)),
                self as *const _ as *const _,
            );
        }
    }

    pub fn insert(&mut self, arg1: Option<U>, arg2: Option<V>) -> bool {
        unsafe {
            KeInsertQueueDpc(
                self.as_mut_ptr(),
                arg1.as_ref()
                    .map_or_else(|| core::ptr::null(), |_| core::mem::transmute(&arg1)),
                arg2.as_ref()
                    .map_or_else(|| core::ptr::null(), |_| core::mem::transmute(&arg2)),
            ) == u8::from(true)
        }
    }

    pub fn remove(&mut self) {}

    pub fn set_importance(&mut self) {}

    unsafe extern "system" fn custom_dpc(
        _dpc: *mut KDPC,
        ctx: *const c_void,
        _arg1: *const c_void,
        _arg2: *const c_void,
    ) {
        let wdu_dpc: &mut WduDpc<T, U, V> = unsafe { core::mem::transmute_copy(&ctx) };

        wdu_dpc.routine.map_or_else(|| (), |rtn| rtn(wdu_dpc));
    }
}

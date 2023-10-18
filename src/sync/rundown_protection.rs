use crate::inner_getters_value;
use windows_sys::Wdk::System::SystemServices::{
    ExAcquireRundownProtection, ExAcquireRundownProtectionEx, ExInitializeRundownProtection,
    ExReInitializeRundownProtection, ExReleaseRundownProtection, ExRundownCompleted,
    ExWaitForRundownProtectionRelease, EX_RUNDOWN_REF, EX_RUNDOWN_REF_0,
};

// TODO: Create cache aware struct
pub struct WduRundownProtection {
    run_ref: EX_RUNDOWN_REF,
}

impl Default for WduRundownProtection {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for WduRundownProtection {
    fn drop(&mut self) {
        self.wait();
    }
}

inner_getters_value!(WduRundownProtection, run_ref, EX_RUNDOWN_REF);

impl WduRundownProtection {
    #[cfg(feature = "const_new")]
    pub const fn const_new() -> Self {
        WduRundownProtection {
            run_ref: EX_RUNDOWN_REF {
                Anonymous: EX_RUNDOWN_REF_0 { Count: 0 },
            },
        }
    }

    pub fn new() -> Self {
        WduRundownProtection {
            run_ref: EX_RUNDOWN_REF {
                Anonymous: EX_RUNDOWN_REF_0 { Count: 0 },
            },
        }
    }

    pub fn init(&mut self) {
        unsafe {
            ExInitializeRundownProtection(self.as_mut_ptr());
        }
    }

    pub fn acquire(&mut self) -> bool {
        unsafe { ExAcquireRundownProtection(self.as_mut_ptr()) == u8::from(true) }
    }

    pub fn acquire_ex(&mut self, count: u32) -> bool {
        unsafe { ExAcquireRundownProtectionEx(self.as_mut_ptr(), count) == u8::from(true) }
    }

    pub fn release(&mut self) {
        unsafe {
            ExReleaseRundownProtection(self.as_mut_ptr());
        }
    }

    pub fn wait(&mut self) {
        unsafe {
            ExWaitForRundownProtectionRelease(self.as_mut_ptr());
        }
    }

    pub fn completed(&mut self) {
        unsafe {
            ExRundownCompleted(self.as_mut_ptr());
        }
    }

    pub fn reinit(&mut self) {
        unsafe {
            ExReInitializeRundownProtection(self.as_mut_ptr());
        }
    }
}

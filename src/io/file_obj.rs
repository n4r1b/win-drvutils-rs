use crate::{
    inner_getters_ptr, strings::unicode::str::WduUnicodeStr,
    strings::unicode::string::WduUnicodeString,
};
use alloc::boxed::Box;
use windows_sys::Wdk::Foundation::FILE_OBJECT;

#[derive(PartialEq)]
pub struct WduFileObject {
    file_object: *mut FILE_OBJECT,
}

inner_getters_ptr!(WduFileObject, file_object, FILE_OBJECT);

impl WduFileObject {
    pub fn is_valid(&self) -> bool {
        !self.file_object.is_null()
    }

    pub fn wrap(fo: *mut FILE_OBJECT) -> Self {
        Self { file_object: fo }
    }

    pub fn file_name(&self) -> WduUnicodeStr {
        unsafe { WduUnicodeStr::from_ptr(&(*self.file_object).FileName) }
    }

    // This could take self as a reference, but I prefer to make clear that this operation will
    // somehow mutate the internal object
    #[cfg(not(feature = "allocator_api"))]
    pub fn set_context<T>(&mut self, context: T) {
        unsafe {
            (*self.file_object).FsContext = Box::into_raw(Box::new(context)) as _;
        }
    }

    #[cfg(feature = "allocator_api")]
    pub fn set_context<T>(&mut self, _context: T) {
        todo!() // Use Box::try_new an return result
    }

    // This could take self as a reference, but I prefer to make clear that this operation will
    // somehow mutate the internal object
    #[cfg(not(feature = "allocator_api"))]
    pub fn set_context2<T>(&mut self, context: T) {
        unsafe {
            (*self.file_object).FsContext2 = Box::into_raw(Box::new(context)) as _;
        }
    }

    #[cfg(feature = "allocator_api")]
    pub fn set_context2<T>(&mut self, _context: T) {
        todo!() // Use Box::try_new and return result
    }

    // TODO: Consider if we want to set FsContext to null, this would require making the method
    //  take a mutable reference
    pub fn context<T>(&self) -> Option<Box<T>> {
        unsafe {
            if (*self.file_object).FsContext.is_null() {
                return None;
            }
            let ctx = (*self.file_object).FsContext as *mut T;

            Some(Box::from_raw(ctx))
        }
    }

    pub fn context2<T>(&self) -> Option<Box<T>> {
        unsafe {
            if (*self.file_object).FsContext2.is_null() {
                return None;
            }
            let ctx = (*self.file_object).FsContext2 as *mut T;

            Some(Box::from_raw(ctx))
        }
    }

    pub fn context_as_ref<T>(&self) -> Option<&T> {
        unsafe {
            if (*self.file_object).FsContext.is_null() {
                return None;
            }
            Some(core::mem::transmute((*self.file_object).FsContext))
        }
    }

    pub fn context_as_mut_ref<T>(&self) -> Option<&mut T> {
        unsafe {
            if (*self.file_object).FsContext.is_null() {
                return None;
            }
            Some(core::mem::transmute((*self.file_object).FsContext))
        }
    }

    pub fn context2_as_ref<T>(&self) -> Option<&T> {
        unsafe {
            if (*self.file_object).FsContext2.is_null() {
                return None;
            }
            Some(core::mem::transmute((*self.file_object).FsContext2))
        }
    }

    pub fn context2_as_mut_ref<T>(&self) -> Option<&mut T> {
        unsafe {
            if (*self.file_object).FsContext2.is_null() {
                return None;
            }
            Some(core::mem::transmute((*self.file_object).FsContext2))
        }
    }
}

//! Simple Kernel Allocator.
//!
//! This module exports a simple memory that allows to configure the Pool tag & Pool type. The
//! module will use ExAllocatePool2 in systems where is available, if it's not the case it will
//! fallback to ExAllocatePoolWithTag. To deallocate it will use ExFreePoolWithTag.
//!
//! The alloc method returns null instead of calling the alloc_error_handler.
//! This gives the opportunity for methods that can handle fallible allocations
//! to avoid panic/abort.
//!
//! ## Remark
//! If using methods that don't support OOM conditions this memory will
//! still panic. This module registers an alloc_error_handler that will panic.
use crate::{
    get_system_routine_addr,
    memory::{PoolFlags, DEFAULT_POOL_TAG},
    WduUnicodeStr,
};

use core::{
    alloc::{GlobalAlloc, Layout},
    ffi::c_void,
};

#[cfg(feature = "allocator_api")]
use core::{
    alloc::{AllocError, Allocator},
    ptr::NonNull,
};

use windows_sys::Wdk::{
    Foundation,
    Foundation::POOL_TYPE,
    System::SystemServices::{ExAllocatePool2, ExAllocatePoolWithTag, ExFreePoolWithTag},
};

// Taken from windows_sys::ExAllocatePool2. We don't use it directly from
// windows-sys MS crate links the function and this function is only available
// from Win10 build 2004
/// Type alias for ExAllocatePool2 function
type ExAllocatePool2Fn =
    extern "system" fn(flags: u64, numberofbytes: usize, tag: u32) -> *mut c_void;

/// Main structure to hold information required by the Simple memory
pub struct SimpleAlloc {
    tag: u32,
    pool_type: POOL_TYPE,
    pool_flags: PoolFlags,
    alloc_pool2: Option<ExAllocatePool2Fn>,
}

// Consider builder pattern. Requires const functions or non-const and consumer calling on init.
impl SimpleAlloc {
    /// Const function to create the memory with default values
    pub const fn const_new() -> Self {
        SimpleAlloc {
            tag: DEFAULT_POOL_TAG,
            pool_type: Foundation::NonPagedPool,
            pool_flags: PoolFlags::PoolFlagNonPaged,
            alloc_pool2: None,
        }
    }

    /// Set the Pool tag to be used by the memory
    pub fn tag(&mut self, tag: u32) {
        self.tag = tag;
    }

    /// Set the Pool type/flags to be used by the memory
    // TODO: Consider wrapping POOL_TYPE in newtype.
    pub fn pool_type(&mut self, pool_type: POOL_TYPE) {
        self.pool_type = pool_type;
        self.pool_flags = PoolFlags::from(pool_type);
    }

    /// Initalize memory
    pub fn init(&mut self) {
        // ExAllocatePool2 in unicode
        let pool2 = WduUnicodeStr::from_slice(&[
            69u16, 120, 65, 108, 108, 111, 99, 97, 116, 101, 80, 111, 111, 108, 50, 00,
        ]);

        self.alloc_pool2 = get_system_routine_addr::<ExAllocatePool2Fn>(&pool2);
    }

    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.alloc_pool2.map_or_else(
            || ExAllocatePoolWithTag(self.pool_type, layout.size(), self.tag) as *mut u8,
            |pfn| pfn(self.pool_flags as u64, layout.size(), self.tag) as *mut u8,
        )
    }

    /// Allocate memory using a specific tag
    pub unsafe fn alloc_with_tag(&self, size: usize, tag: u32) -> *mut u8 {
        self.alloc_pool2.map_or_else(
            || ExAllocatePoolWithTag(self.pool_type, size, tag) as *mut u8,
            |pfn| pfn(self.pool_flags as u64, size, tag) as *mut u8,
        )
    }

    /// Free memory using a specific tag
    pub unsafe fn free_with_tag(&self, ptr: *mut u8, tag: u32) {
        ExFreePoolWithTag(ptr as _, tag);
    }
}

#[cfg(feature = "allocator_api")]
unsafe impl Allocator for SimpleAlloc {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let pool = unsafe { self.alloc(layout) };

        if pool.is_null() {
            return Err(AllocError);
        }

        let slice = unsafe { core::slice::from_raw_parts_mut(pool, layout.size()) };
        Ok(unsafe { NonNull::new_unchecked(slice) })
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, _layout: Layout) {
        ExFreePoolWithTag(ptr.as_ptr() as _, self.tag);
    }
}

unsafe impl GlobalAlloc for SimpleAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pool = self.alloc(layout);
        pool as _
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        ExFreePoolWithTag(ptr as _, self.tag);
    }
}

/// Handler for OOM conditions
#[alloc_error_handler]
fn alloc_error(layout: Layout) -> ! {
    panic!("allocation failed: {:?}", layout);
}

pub struct PagedPool;
pub struct NonPagedPool;

impl PagedPool {
    pub unsafe fn alloc(size: usize) -> *mut u8 {
        ExAllocatePool2(PoolFlags::PoolFlagPaged.into(), size, DEFAULT_POOL_TAG) as *mut u8
    }

    /// Allocate memory using a specific tag
    pub unsafe fn alloc_with_tag(size: usize, tag: u32) -> *mut u8 {
        ExAllocatePool2(PoolFlags::PoolFlagPaged.into(), size, tag) as *mut u8
    }

    /// Free memory using a specific tag
    pub unsafe fn free_with_tag(ptr: *mut u8, tag: u32) {
        ExFreePoolWithTag(ptr as _, tag);
    }
}

impl NonPagedPool {
    pub unsafe fn alloc(size: usize) -> *mut u8 {
        ExAllocatePool2(PoolFlags::PoolFlagNonPaged.into(), size, DEFAULT_POOL_TAG) as *mut u8
    }

    /// Allocate memory using a specific tag
    pub unsafe fn alloc_with_tag(size: usize, tag: u32) -> *mut u8 {
        ExAllocatePool2(PoolFlags::PoolFlagNonPaged.into(), size, tag) as *mut u8
    }

    /// Free memory using a specific tag
    pub unsafe fn free_with_tag(ptr: *mut u8, tag: u32) {
        ExFreePoolWithTag(ptr as _, tag);
    }
}

#[cfg(feature = "allocator_api")]
unsafe impl Allocator for PagedPool {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let pool = unsafe { PagedPool::alloc(layout.size()) };

        if pool.is_null() {
            return Err(AllocError);
        }

        let slice = unsafe { core::slice::from_raw_parts_mut(pool, layout.size()) };
        Ok(unsafe { NonNull::new_unchecked(slice) })
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, _layout: Layout) {
        ExFreePoolWithTag(ptr.as_ptr() as _, DEFAULT_POOL_TAG);
    }
}

#[cfg(feature = "allocator_api")]
unsafe impl Allocator for NonPagedPool {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let pool = unsafe { NonPagedPool::alloc(layout.size()) };

        if pool.is_null() {
            return Err(AllocError);
        }

        let slice = unsafe { core::slice::from_raw_parts_mut(pool, layout.size()) };
        Ok(unsafe { NonNull::new_unchecked(slice) })
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, _layout: Layout) {
        ExFreePoolWithTag(ptr.as_ptr() as _, DEFAULT_POOL_TAG);
    }
}

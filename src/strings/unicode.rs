use alloc::{
    slice,
    string::{String, ToString},
};
use core::fmt::{Debug, Display, Formatter};
use snafu::Snafu;

use windows_sys::Win32::Foundation::{NTSTATUS, UNICODE_STRING};

#[derive(Debug, Snafu)]
pub enum WduUnicodeError {
    #[snafu(display("Unable to validate UNICODE_STRING"))]
    Invalid,
    #[snafu(display("RtlCreateUnicodeString failed"))]
    CreateError,
    #[snafu(display("Insufficient memory"))]
    InsufficientResources,
    #[snafu(display("NTSTATUS: {status}"))]
    NtStatus { status: NTSTATUS },
}

pub type WduUnicodeResult<T> = Result<T, WduUnicodeError>;

pub mod str {
    use super::string::WduUnicodeString;
    use super::*;
    use core::borrow::Borrow;

    #[derive(Default, PartialEq)]
    pub struct WduUnicodeStr<'a> {
        pub slice: &'a [u16],
    }

    impl PartialEq<UNICODE_STRING> for WduUnicodeStr<'_> {
        fn eq(&self, other: &UNICODE_STRING) -> bool {
            let slice = unsafe { slice::from_raw_parts(other.Buffer, other.Length as usize) };

            self.slice == slice
        }
    }

    impl PartialEq<WduUnicodeString> for WduUnicodeStr<'_> {
        fn eq(&self, other: &WduUnicodeString) -> bool {
            self.slice == other.as_slice()
        }
    }

    impl PartialEq<str> for WduUnicodeStr<'_> {
        fn eq(&self, other: &str) -> bool {
            self.to_string() == other
        }
    }

    impl PartialEq<&str> for WduUnicodeStr<'_> {
        fn eq(&self, other: &&str) -> bool {
            self.to_string() == other.to_string()
        }
    }

    impl Into<UNICODE_STRING> for WduUnicodeStr<'_> {
        fn into(self) -> UNICODE_STRING {
            self.as_unicode_string()
        }
    }

    impl Into<UNICODE_STRING> for &WduUnicodeStr<'_> {
        fn into(self) -> UNICODE_STRING {
            self.as_unicode_string()
        }
    }

    impl Display for WduUnicodeStr<'_> {
        fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
            write!(f, "{}", String::from_utf16_lossy(self.slice))
        }
    }

    impl<'a> WduUnicodeStr<'a> {
        fn to_unicode_len(len: usize) -> u16 {
            (len * core::mem::size_of::<u16>()) as u16
        }

        pub fn from_slice<'b>(source: &'b [u16]) -> WduUnicodeStr<'a>
        where
            'b: 'a,
        {
            Self { slice: source }
        }

        pub fn from_ptr(source: *const UNICODE_STRING) -> WduUnicodeStr<'a> {
            if source.is_null() {
                return Self::default();
            }

            Self {
                slice: unsafe {
                    core::slice::from_raw_parts(
                        (*source).Buffer as *const _,
                        ((*source).MaximumLength / 2) as usize,
                    )
                },
            }
        }

        pub fn to_owned(&self) -> WduUnicodeResult<WduUnicodeString> {
            Ok(WduUnicodeString::try_from(self.slice)?)
        }

        pub fn as_unicode_string(&self) -> UNICODE_STRING {
            UNICODE_STRING {
                Length: Self::to_unicode_len(self.slice.len()),
                MaximumLength: Self::to_unicode_len(self.slice.len()),
                Buffer: self.slice.as_ptr() as *mut _,
            }
        }

        pub fn is_empty(&self) -> bool {
            self.slice.is_empty()
        }
    }
}

#[cfg(not(feature = "unicode_as_vec"))]
pub mod string {
    use crate::{
        inner_getters_value,
        memory::{
            pool::{NonPagedPool, PagedPool},
            PoolFlags,
            PoolFlags::{PoolFlagNonPaged, PoolFlagPaged},
        },
        strings::unicode::{str::WduUnicodeStr, WduUnicodeError, WduUnicodeResult},
    };

    use widestring::{U16CStr, Utf16Str};

    use core::{
        borrow::Borrow,
        fmt::{Debug, Display, Formatter},
        hash::{Hash, Hasher},
        str::FromStr,
    };

    use alloc::{
        boxed::Box,
        slice,
        string::{String, ToString},
        vec,
        vec::Vec,
    };

    use windows_sys::{
        // Not sure why this is inside Storage::FileSystem, let's use it anyway
        Wdk::{
            Storage::FileSystem::{
                FsRtlIsNameInExpression, RtlDowncaseUnicodeString, RtlDuplicateUnicodeString,
                RtlValidateUnicodeString,
            },
            System::SystemServices::{
                ExFreePoolWithTag, RtlCompareUnicodeString, RtlCopyUnicodeString,
                RtlInt64ToUnicodeString, RtlIntegerToUnicodeString, RtlPrefixUnicodeString,
                RtlSuffixUnicodeString, RtlUTF8StringToUnicodeString, RtlUpcaseUnicodeString,
            },
        },
        Win32::{
            Foundation::{NTSTATUS, STATUS_INVALID_PARAMETER, STATUS_SUCCESS, UNICODE_STRING},
            System::{
                Kernel::STRING, Memory::RtlCompareMemory, WindowsProgramming::RtlFreeUnicodeString,
            },
        },
    };

    #[repr(u32)]
    pub enum CopyFlags {
        DestNullTerminated = 0,
        DestNonNullTerminated = 1,
        CreateBufferOnEmtpySource = 3,
    }

    const MAX_LEN_U32_STRING: u16 = 0x15;
    const MAX_LEN_U64_STRING: u16 = 0x30;
    const STRING_ALLOC_TAG: u32 = u32::from_ne_bytes(*b"WDUs");

    #[cfg(feature = "try_non_paged")]
    const POOL_TYPE: PoolFlags = PoolFlags::PoolFlagNonPaged;

    #[cfg(not(feature = "try_non_paged"))]
    const POOL_TYPE: PoolFlags = PoolFlags::PoolFlagPaged;

    #[derive(Debug, PartialEq, Copy, Clone)]
    enum AllocType {
        Os,
        Rust,
        Pool,
    }

    // TODO: Consider defining two separate UNICODE_STRING, one with PWSTR and another one with PCWSTR.
    //  have some trait so we can constraint functions to either one, the other or both.
    // TODO: Consider having a WduUnicodeStr to have UNICODE_STRING where we don't own the buffer.
    pub struct WduUnicodeString {
        string: UNICODE_STRING,
        alloc: Option<AllocType>, // If allocator_api stable we could store Allocator here
    }

    impl Default for WduUnicodeString {
        fn default() -> Self {
            Self {
                string: UNICODE_STRING {
                    Length: 0,
                    MaximumLength: 0,
                    Buffer: core::ptr::null_mut(),
                },
                alloc: None,
            }
        }
    }

    impl Drop for WduUnicodeString {
        fn drop(&mut self) {
            self.alloc.map_or_else(
                || (),
                |alloc| match alloc {
                    AllocType::Os => unsafe { RtlFreeUnicodeString(self.as_mut_ptr()) },
                    AllocType::Rust => unsafe {
                        let _ = Box::from_raw(self.string.Buffer);
                    },
                    AllocType::Pool => unsafe {
                        WduUnicodeString::free_buffer(self.string.Buffer as *mut _)
                    },
                },
            );
        }
    }

    impl PartialEq<UNICODE_STRING> for WduUnicodeString {
        fn eq(&self, other: &UNICODE_STRING) -> bool {
            self.compare(&WduUnicodeString::wrap(other), false)
        }
    }

    impl PartialEq<Self> for WduUnicodeString {
        fn eq(&self, other: &Self) -> bool {
            self.compare(other, false)
        }
    }

    impl PartialEq<str> for WduUnicodeString {
        fn eq(&self, other: &str) -> bool {
            self.to_string() == other
        }
    }

    impl PartialEq<&str> for WduUnicodeString {
        fn eq(&self, other: &&str) -> bool {
            self.to_string() == other.to_string()
        }
    }

    impl<'a> PartialEq<WduUnicodeStr<'a>> for WduUnicodeString {
        fn eq(&self, other: &WduUnicodeStr<'a>) -> bool {
            self.as_slice() == other.slice
        }
    }

    impl Hash for WduUnicodeString {
        fn hash<H: Hasher>(&self, _state: &mut H) {
            todo!()
        }
    }

    impl TryFrom<Vec<u16>> for WduUnicodeString {
        type Error = WduUnicodeError;

        fn try_from(value: Vec<u16>) -> Result<Self, Self::Error> {
            WduUnicodeString::create(&value, POOL_TYPE)
        }
    }

    impl TryFrom<&[u16]> for WduUnicodeString {
        type Error = WduUnicodeError;

        fn try_from(value: &[u16]) -> Result<Self, Self::Error> {
            WduUnicodeString::create(value, POOL_TYPE)
        }
    }

    impl TryFrom<&Utf16Str> for WduUnicodeString {
        type Error = WduUnicodeError;

        fn try_from(value: &Utf16Str) -> Result<Self, Self::Error> {
            WduUnicodeString::create(value.as_slice(), POOL_TYPE)
        }
    }

    impl TryFrom<&U16CStr> for WduUnicodeString {
        type Error = WduUnicodeError;

        fn try_from(value: &U16CStr) -> Result<Self, Self::Error> {
            WduUnicodeString::create(value.as_slice(), POOL_TYPE)
        }
    }

    impl TryFrom<&[u8]> for WduUnicodeString {
        type Error = WduUnicodeError;

        fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
            WduUnicodeString::try_from_utf8(value)
        }
    }

    impl TryFrom<u32> for WduUnicodeString {
        type Error = WduUnicodeError;

        fn try_from(value: u32) -> Result<Self, Self::Error> {
            WduUnicodeString::try_from_u32(value, 10)
        }
    }

    impl TryFrom<u64> for WduUnicodeString {
        type Error = WduUnicodeError;

        fn try_from(value: u64) -> Result<Self, Self::Error> {
            WduUnicodeString::try_from_u64(value, 10)
        }
    }

    impl TryFrom<&str> for WduUnicodeString {
        type Error = WduUnicodeError;

        fn try_from(value: &str) -> Result<Self, Self::Error> {
            Ok(WduUnicodeString::from_str(value)?)
        }
    }

    impl FromStr for WduUnicodeString {
        type Err = WduUnicodeError;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            WduUnicodeString::try_from_str(s, POOL_TYPE)
        }
    }

    impl Clone for WduUnicodeString {
        fn clone(&self) -> Self {
            self.copy()
        }
    }

    impl Debug for WduUnicodeString {
        fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
            write!(
                f,
                "Len {}, MaxLen {}, Buffer {:?}. [Rust allocated: {:?}]",
                self.string.Length, self.string.MaximumLength, self.string.Buffer, self.alloc
            )
        }
    }

    impl Display for WduUnicodeString {
        fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
            let string = self.as_slice();

            write!(f, "{}", String::from_utf16_lossy(string))
        }
    }

    // TODO: Implement set of Into(widestring) + TryInto

    inner_getters_value!(WduUnicodeString, string, UNICODE_STRING);

    // TODO: implement ntstrsafe validations!
    impl WduUnicodeString {
        #[cfg(feature = "const_new")]
        pub const fn const_new() -> Self {
            Self {
                string: UNICODE_STRING {
                    Length: 0,
                    MaximumLength: 0,
                    Buffer: core::ptr::null_mut(),
                },
                alloc: None,
            }
        }

        fn to_unicode_len(len: usize) -> u16 {
            (len * core::mem::size_of::<u16>()) as u16
        }

        // Could be a method of the class and set alloc to Some(AllocType::Pool) on success
        unsafe fn alloc_buffer(len: u16) -> WduUnicodeResult<*mut u16> {
            let buffer;
            #[cfg(feature = "try_non_paged")]
            {
                buffer = NonPagedPool::alloc_with_tag(len as usize, STRING_ALLOC_TAG) as *mut u16;
            }

            #[cfg(not(feature = "try_non_paged"))]
            {
                buffer = PagedPool::alloc_with_tag(len as usize, STRING_ALLOC_TAG) as *mut u16;
            }

            if buffer.is_null() {
                return Err(WduUnicodeError::InsufficientResources);
            }

            Ok(buffer)
        }

        unsafe fn free_buffer(ptr: *mut u8) {
            // NonPaged and Paged both free using ExFreePoolWithTag
            ExFreePoolWithTag(ptr as *mut _, STRING_ALLOC_TAG);
        }

        pub fn is_empty(&self) -> bool {
            if self.string.Length == 0 || self.string.MaximumLength == 0 {
                return true;
            }
            false
        }

        pub fn validate(&self) -> WduUnicodeResult<()> {
            if unsafe { RtlValidateUnicodeString(0, self.as_ptr()) } == STATUS_INVALID_PARAMETER {
                return Err(WduUnicodeError::Invalid);
            }

            Ok(())
        }

        pub fn len(&self) -> u16 {
            self.string.Length / core::mem::size_of::<u16>() as u16
        }

        pub fn bytes(&self) -> u16 {
            self.string.Length
        }

        pub fn capacity(&self) -> u16 {
            self.string.MaximumLength
        }

        pub fn is_rust_alloc(&self) -> bool {
            self.alloc
                .map_or_else(|| false, |alloc| alloc == AllocType::Rust)
        }

        pub fn is_os_alloc(&self) -> bool {
            self.alloc
                .map_or_else(|| false, |alloc| alloc == AllocType::Os)
        }

        pub fn is_pool_alloc(&self) -> bool {
            self.alloc
                .map_or_else(|| false, |alloc| alloc == AllocType::Pool)
        }

        pub fn as_slice(&self) -> &[u16] {
            unsafe { slice::from_raw_parts(self.string.Buffer as *const _, self.len().into()) }
        }

        pub fn wrap(source: *const UNICODE_STRING) -> Self {
            if source.is_null() {
                return Self::default();
            }

            WduUnicodeString {
                string: unsafe { *source },
                alloc: None,
            }
        }

        pub fn new(source: &UNICODE_STRING) -> Self {
            // Consider directly copying the UNICODE_STRING
            Self::wrap(source).clone()
        }

        pub fn take(source: UNICODE_STRING) -> Self {
            WduUnicodeString {
                string: source,
                alloc: Some(AllocType::Os),
            }
        }

        pub fn create(source: &[u16], pool_type: PoolFlags) -> WduUnicodeResult<Self> {
            let mut wdu_string = WduUnicodeString::default();
            let len = Self::to_unicode_len(source.len());

            let buffer: *mut u16 = match pool_type {
                PoolFlags::PoolFlagNonPaged => unsafe {
                    NonPagedPool::alloc_with_tag(len as usize, STRING_ALLOC_TAG) as *mut _
                },
                PoolFlags::PoolFlagPaged => unsafe {
                    PagedPool::alloc_with_tag(len as usize, STRING_ALLOC_TAG) as *mut _
                },
                _ => panic!("Invalid PoolFlags for WduUnicodeString allocation"),
            };

            if buffer.is_null() {
                return Err(WduUnicodeError::InsufficientResources);
            }

            unsafe {
                core::ptr::copy(source.as_ptr(), buffer, len as usize);
            }

            wdu_string.string.Length = len;
            wdu_string.string.MaximumLength = len;
            wdu_string.string.Buffer = buffer;

            wdu_string.alloc = Some(AllocType::Pool);
            Ok(wdu_string)
        }

        fn try_from_utf8(source: &[u8]) -> WduUnicodeResult<Self> {
            // TODO: Once we have a ansi String class prefer that
            let string = STRING {
                Length: source.len() as u16,
                MaximumLength: source.len() as u16,
                Buffer: source.as_ptr() as *mut _,
            };

            let mut wdu_string = WduUnicodeString::default();

            let status = unsafe {
                RtlUTF8StringToUnicodeString(
                    wdu_string.as_mut_ptr(),
                    string.borrow(),
                    u8::from(true),
                )
            };

            if status != STATUS_SUCCESS {
                return Err(WduUnicodeError::NtStatus { status });
            }

            wdu_string.alloc = Some(AllocType::Os);

            Ok(wdu_string)
        }

        fn try_from_str(source: &str, pool_type: PoolFlags) -> WduUnicodeResult<Self> {
            let u16_size = source.encode_utf16().count() * core::mem::size_of::<u16>();

            let mut buffer = Vec::new();
            buffer
                .try_reserve(u16_size) // alloc size for null-terminator
                .or_else(|_| Err(WduUnicodeError::InsufficientResources))?;
            buffer.extend(source.encode_utf16());

            Ok(Self::create(buffer.as_slice(), pool_type)?)
        }

        pub fn try_from_u32(val: u32, base: u32) -> WduUnicodeResult<Self> {
            let mut wdu_string = WduUnicodeString::default();

            wdu_string.string.Length = MAX_LEN_U32_STRING;
            wdu_string.string.MaximumLength = MAX_LEN_U32_STRING;
            wdu_string.string.Buffer = unsafe { Self::alloc_buffer(MAX_LEN_U32_STRING)? };
            wdu_string.alloc = Some(AllocType::Pool);

            let status = unsafe { RtlIntegerToUnicodeString(val, base, wdu_string.as_mut_ptr()) };

            if status != STATUS_SUCCESS {
                return Err(WduUnicodeError::NtStatus { status });
            }

            Ok(wdu_string)
        }

        pub fn try_from_u64(val: u64, base: u32) -> WduUnicodeResult<Self> {
            let mut wdu_string = WduUnicodeString::default();

            wdu_string.string.Length = MAX_LEN_U64_STRING;
            wdu_string.string.MaximumLength = MAX_LEN_U64_STRING;
            wdu_string.string.Buffer = unsafe { Self::alloc_buffer(MAX_LEN_U64_STRING)? };
            wdu_string.alloc = Some(AllocType::Pool);

            let status = unsafe { RtlInt64ToUnicodeString(val, base, wdu_string.as_mut_ptr()) };

            if status != STATUS_SUCCESS {
                return Err(WduUnicodeError::NtStatus { status });
            }

            Ok(wdu_string)
        }

        fn copy(&self) -> Self {
            let mut dest = WduUnicodeString::default();

            // Make this OOM capable (or maybe just leave that to try_copy??)
            let buffer = Box::new(vec![0u16; self.len() as usize]);

            dest.string.Length = self.bytes();
            dest.string.MaximumLength = self.capacity();
            dest.string.Buffer = Box::into_raw(buffer) as *mut _;

            unsafe { RtlCopyUnicodeString(dest.as_mut_ptr(), self.as_ptr()) };

            dest.alloc = Some(AllocType::Rust);
            dest
        }

        fn try_copy(&self) -> WduUnicodeResult<Self> {
            todo!()
        }

        pub fn duplicate(&self, flags: CopyFlags) -> WduUnicodeResult<Self> {
            let mut dest = WduUnicodeString::default();
            let status = unsafe {
                RtlDuplicateUnicodeString(flags as u32, self.as_ptr(), dest.as_mut_ptr())
            };

            if status != STATUS_SUCCESS {
                return Err(WduUnicodeError::NtStatus { status });
            }

            dest.alloc = Some(AllocType::Os);

            Ok(dest)
        }

        pub fn compare(&self, other: &Self, case_insensitive: bool) -> bool {
            unsafe {
                RtlCompareUnicodeString(self.as_ptr(), other.as_ptr(), u8::from(case_insensitive))
                    == 0
            }
        }

        pub fn np_compare(&self, other: &Self) -> bool {
            if self.string.Length != other.string.Length
                || self.string.MaximumLength != other.string.MaximumLength
            {
                return false;
            }

            unsafe {
                RtlCompareMemory(
                    self.string.Buffer as *const _,
                    other.string.Buffer as *const _,
                    self.string.Length as usize,
                ) == self.string.Length as usize
            }
        }

        // TODO: transform in-place
        pub fn to_lower(&mut self) -> WduUnicodeResult<()> {
            todo!()
        }

        // TODO: transform in-place
        pub fn to_upper(&mut self) -> WduUnicodeResult<()> {
            todo!()
        }

        pub fn new_lower(&self) -> WduUnicodeResult<Self> {
            let mut dest = WduUnicodeString::default();

            // Let the OS allocate for us
            let status = unsafe {
                RtlDowncaseUnicodeString(dest.as_mut_ptr(), self.as_ptr(), u8::from(true))
            };

            if status != STATUS_SUCCESS {
                return Err(WduUnicodeError::NtStatus { status });
            }

            dest.alloc = Some(AllocType::Os);
            Ok(dest)
        }

        pub fn new_upper(&self) -> WduUnicodeResult<Self> {
            let mut dest = WduUnicodeString::default();

            // Let the OS allocate for us
            let status =
                unsafe { RtlUpcaseUnicodeString(dest.as_mut_ptr(), self.as_ptr(), u8::from(true)) };

            if status != STATUS_SUCCESS {
                return Err(WduUnicodeError::NtStatus { status });
            }

            dest.alloc = Some(AllocType::Os);
            Ok(dest)
        }

        pub fn is_suffix(&self, source: &WduUnicodeString, case_insensitive: bool) -> bool {
            unsafe {
                RtlSuffixUnicodeString(source.as_ptr(), self.as_ptr(), u8::from(case_insensitive))
                    == u8::from(true)
            }
        }

        pub fn is_prefix(&self, source: &WduUnicodeString, case_insensitive: bool) -> bool {
            unsafe {
                RtlPrefixUnicodeString(source.as_ptr(), self.as_ptr(), u8::from(case_insensitive))
                    == u8::from(true)
            }
        }

        pub fn match_expression(
            &self,
            expression: &WduUnicodeString,
            case_insensitive: bool,
        ) -> bool {
            unsafe {
                FsRtlIsNameInExpression(
                    expression.as_ptr(),
                    self.as_ptr(),
                    u8::from(case_insensitive),
                    core::ptr::null(),
                ) == u8::from(true)
            }
        }

        pub fn contains(&self, needle: &WduUnicodeString, case_insensitive: bool) -> bool {
            // Let's not even bother to compare if needle is bigger than self.len
            if self.bytes() < needle.bytes() {
                return false;
            }

            // This will allocate/free memory not a big deal but we should probably create another
            // method that's able to do the same without allocating memory
            if case_insensitive {
                self.to_string()
                    .to_lowercase()
                    .contains(&needle.to_string().to_lowercase())
            } else {
                self.to_string().contains(&needle.to_string())
            }
        }

        // TODO: add more functions Hash, Append, SidToUnicode, OemToUnicode
    }
}

#[cfg(feature = "unicode_as_vec")]
pub mod string {
    use crate::strings::unicode::WduUnicodeError;
    use alloc::vec::Vec;
    use windows_sys::Win32::Foundation::UNICODE_STRING;

    #[derive(Default)]
    pub struct WduUnicodeString {
        buffer: Vec<u16>,
    }

    impl Into<UNICODE_STRING> for WduUnicodeString {
        fn into(self) -> UNICODE_STRING {
            todo!()
        }
    }

    // impl TryFrom
    impl TryFrom<&[u16]> for WduUnicodeString {
        type Error = WduUnicodeError;

        fn try_from(value: &[u16]) -> Result<Self, Self::Error> {
            Ok(WduUnicodeString::from_slice(value))
        }
    }

    impl WduUnicodeString {
        pub fn new(string: &UNICODE_STRING) -> Self {
            Self {
                buffer: unsafe {
                    Vec::from_raw_parts(
                        string.Buffer,
                        string.Length as usize,
                        string.MaximumLength as usize,
                    )
                },
            }
        }

        pub fn as_ptr(&self) -> *const UNICODE_STRING {
            todo!()
        }

        pub fn as_mut_ptr(&self) -> *mut UNICODE_STRING {
            todo!()
        }

        pub fn get(&self) -> UNICODE_STRING {
            todo!()
        }

        // We could use to_vec_in to provide different allocators
        pub fn from_slice(slice: &[u16]) -> Self {
            Self {
                buffer: slice.to_vec(),
            }
        }

        pub fn as_slice(&self) -> &[u16] {
            self.buffer.as_slice()
        }
    }
}

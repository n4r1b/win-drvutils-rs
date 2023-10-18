#![no_std]
#![allow(internal_features)]
#![feature(lang_items)]
#![allow(non_snake_case)]
extern crate alloc;

use alloc::{format, string::ToString, vec};
use core::{ffi::c_void, str::FromStr};
use kernel_log::KernelLogger;
use log::LevelFilter;
use widestring::{u16cstr, utf16str};

use win_drvutils_rs::{
    bug_check,
    memory::{pool::SimpleAlloc, PoolFlags::PoolFlagNonPaged},
    strings::unicode::{str::WduUnicodeStr, string::WduUnicodeString, WduUnicodeResult},
};

use windows_sys::{
    Wdk::System::SystemServices::RtlCompareUnicodeString, Win32::Foundation::UNICODE_STRING,
};

#[global_allocator]
static mut GLOBAL: SimpleAlloc = SimpleAlloc::const_new();

#[export_name = "_fltused"]
static _FLTUSED: i32 = 0;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    bug_check(info, None, None, None, None);
    loop {}
}

#[lang = "eh_personality"]
extern "C" fn eh_personality() {}

#[no_mangle]
extern "system" fn __CxxFrameHandler3(_: *mut u8, _: *mut u8, _: *mut u8, _: *mut u8) -> i32 {
    unimplemented!()
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "system" fn DriverEntry(
    _driver_object: *mut c_void,
    reg_path: *const UNICODE_STRING,
) -> i32 {
    KernelLogger::init(LevelFilter::Info).expect("Failed to initialize logger");
    unsafe {
        GLOBAL.init();
    }

    if let Err(err) = test_unicode(reg_path) {
        panic!("Failed to test Unicode module. Error: {:?}", err);
    }

    0
}

fn test_unicode(reg_path: *const UNICODE_STRING) -> WduUnicodeResult<()> {
    let hello_str = "Hello";
    let x: [u16; 5] = [0x48, 0x65, 0x6C, 0x6C, 0x6f]; // Hello

    //
    // WduUnicodeStr Tests
    //
    let hello = WduUnicodeStr::from_slice(&x);
    let reg_path_str = WduUnicodeStr::from_ptr(reg_path);
    let reg_path_owned = reg_path_str.to_owned()?;

    // Test to_owned works as expected and PartialEq<WduUnicodeStr, WduUnicodeString>
    assert!(hello == hello_str);
    assert!(reg_path_str == reg_path_owned);

    // Test WduUnicodeString::as_ptr properly returns a PCUNICODE_STRING
    let res = unsafe { RtlCompareUnicodeString(reg_path, &reg_path_str.into(), u8::from(false)) };

    assert!(res == 0);

    //
    // WduUnicodeString Tests
    //
    let empty = WduUnicodeString::default();
    assert!(empty.is_empty());
    assert!(empty.validate().is_ok());

    // Test WduUnicodeString.Lenght = 1 is invalid, based on code of RtlValidateUnicodeString
    let mut invalid = WduUnicodeString::default();
    unsafe {
        (*invalid.as_mut_ptr()).Length = 1;
    }
    assert!(invalid.validate().is_err());

    // Test create from NonPagedPool uses PoolAlloc
    let hello_alloc = WduUnicodeString::create(&x, PoolFlagNonPaged)?;
    assert!(hello_alloc.is_pool_alloc());

    // Test WduUnicodeString clone works and strings are equal
    let hello_clone = hello_alloc.clone();
    assert!(hello_clone == hello_alloc);

    // Test TryFrom<str>
    let from_str = WduUnicodeString::from_str("from_str")?;
    assert!(from_str.to_string() == "from_str");

    // Test TryFrom<u64>
    let from_u64 = WduUnicodeString::try_from(0xDEADBEEFu64)?;
    assert!(from_u64.to_string() == format!("{}", 0xDEADBEEFu64));

    // Test try_from_32 with Hexadecimal base
    let from_u32 = WduUnicodeString::try_from_u32(0xCAFEu32, 16)?;
    assert!(from_u32.to_string() == "CAFE");

    let n4r1b_str = "n4r1B";
    let y = vec![0x6Eu16, 0x34, 0x72, 0x31, 0x42]; // n4r1B

    // Test TryFrom<Vec<u16>>
    let n4r1B = WduUnicodeString::try_from(y.clone())?;
    assert!(n4r1B.to_string() == n4r1b_str);

    let n4r1b_u16cstr = u16cstr!("N4R1B");
    let n4r1b_upcase = WduUnicodeString::try_from(n4r1b_u16cstr)?;

    // Test TryFrom<U16CStr>
    assert!(n4r1b_upcase.as_slice() == n4r1b_u16cstr.as_slice());
    // Compare N4R1B == n4r1B (case sensitive)
    assert!(n4r1b_upcase.compare(&n4r1B, false) == false);
    // Compare N4R1B == n4r1B (case insensitive)
    assert!(n4r1b_upcase.compare(&n4r1B, true) == true);

    // Test to_lower and Non-paged compare works (Assume both strings are in NonPaged memory)
    let n4r1b = WduUnicodeString::try_from(utf16str!("n4r1b"))?;
    let n4r1b_lower = n4r1b_upcase.new_lower()?;
    assert!(n4r1b_lower.np_compare(&n4r1b) == true);

    let z = "services";
    let services = WduUnicodeString::try_from(z)?;
    let reg_path_string = WduUnicodeString::wrap(reg_path);

    assert!(reg_path_string.contains(&services, false) == false);
    assert!(reg_path_string.contains(&services, true) == true);

    Ok(())
}

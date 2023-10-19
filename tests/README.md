## win-drvutils-rs tests
This directory serves as a dedicated space for testing each individual component of the `win-drvutils-rs` project.

Currently, we conduct component testing by creating simple drivers that register a `panic_handler` capable of 
triggering the [WDU BugCheck](https://github.com/n4r1b/win-drvutils-rs/blob/master/src/lib.rs#L164-L184). This 
approach allows us to use `assert!()` to validate that the components are functioning as expected. 

If running these drivers with a WinDBG attached we can inspect the Arg1 of the BugCheck to determine which assertion 
has triggered the BSOD:

```
KDTARGET: Refreshing KD connection

*** Fatal System Error: 0x06941393
                       (0xFFFFAA8D504BEB80,0x0000000000000000,0x0000000000000000,0x0000000000000000)

Break instruction exception - code 80000003 (first chance)

A fatal system error has occurred.
Debugger entered on first try; Bugcheck callbacks have not been invoked.

A fatal system error has occurred.

For analysis of this file, run !analyze -v

1: kd> da /c100 0xFFFFAA8D504BEB80
ffffaa8d`504beb80  "panicked at src/lib.rs:73:5:.assertion failed: hello != hello_str"
```

The drivers in this directory are intentionally kept simple and should ideally require minimal interaction or none 
at all. Whenever possible, they should be straightforward enough to test all expectations within the `DriverEntry` 
and `DriverUnload` functions.

In our development workflow, it's our goal to run this set of test drivers on a VM before introducing new code to the repository.

Any contributions to this testing framework are greatly appreciated, as they play a crucial role in ensuring the 
reliability and stability of `win-drvutils-rs`.

### TODO
- Figure out the best way to write UTs

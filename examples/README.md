## win-drvutils-rs examples
This directory keeps a collection of samples of drivers written using the `win-drvutils-rs` crate.

At the moment the sample drivers are being taken from the [windows-driver-samples](https://github.com/microsoft/Windows-driver-samples).
The examples are copies almost 1-1 to the C/C++ code from the sample driver, this is of course not ideal but is 
just for the purpose of keeping the examples as similar as the original. If writting a new driver from scratch in 
Rust the design will most likely differ from what you would write when writting a driver in C or C++. 

If you have any request of a sample driver that you'd like being implemented using `win-drvutils-rs` please open an 
Issue requesting for it and I'll try to implement it.

### TODO
- Add more examples (Toaster!)
- Write executables to test drivers
- Update drivers to use `wdk-build` from [windows-drivers-rs](https://github.com/microsoft/windows-drivers-rs)

## Rust Windows Driver Utils 
Welcome to the `win-drvutils-rs` repository, a collection of utilities designed with the purpose of simplifying and 
enhancing the development of Windows drivers using Rust.

> This crate is in its alpha development stage. Any assistance with code contributions or bug discovery is greatly
> appreciated.

### Overview
This repository aims to simplify the development of Windows drivers in Rust by providing a set of safe-wrappers over
Kernel objects and functions.

> **Remark:** While the library aims to enable clients to write code without resorting to unsafe blocks, there are 
> specific scenarios where using unsafe blocks is still necessary at the moment.

This library follows an approach similar to WDF in how it exposes library objects that serve as containers for WDM objects. 
These library objects provide functions that allow clients to perform operations in the kernel.

For objects that correspond one-to-one with a WDM object, the library always offers a means to obtain a pointer or 
reference to the underlying object. However, it's important to note that not all objects have a direct one-to-one 
relationship with a WDM object. In some cases, these objects serve as containers for multiple pieces of information, 
and in such cases, the only way to interact with the object is through its public methods.

Similar to how WDF objects use the `Wdf` prefix, in this library wrappers will always use the prefix `Wdu (Windows 
Driver Utils)` this will also serve as a way to differentiate objects from this library with the original objects.

> **Remark:** Even thou the library ships with a rust-toolchain.toml that sets the nightly channel. It should be 
> possible to build the library using the stable channel by just removing the `alloc_error_handler` feature (Used 
> only by the allocator).

### Memory Allocation
When allocating memory, the library primarily leverages the kernel to handle the allocation process whenever 
possible (e.g.: [RtlCreateUnicodeString](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntifs/nf-ntifs-rtlcreateunicodestring)).
In cases where kernel-based allocation is not feasible, the library defaults to using `alloc::boxed::Box` for memory allocation. 
Additionally, the library offers support for the `Allocator API` through a feature flag, enabling fallible allocations 
that return a `Result`.

### Objects Lifetime
In kernel development, it's common to work with long-lived objects that extend beyond the scope of a function. 
These objects often need to be accessible from various parts of the system and across multiple threads. These 
requirements don't always align seamlessly with Rust's lifetime rules.

Some of the common options to address these needs are:
- Using a `static mut` object, which requires accessing it within an `unsafe` block to ensure synchronization. 
  However, this approach has a drawback - the Drop trait's `drop` method won't be called when the object is no 
  longer  needed. To overcome this, a `cleanup` method should be called explicitly before unloading the driver to
  release any resources held by the object.
- Allocating memory on the heap and having a means to retrieve it when necessary. In Rust, this is can be achieved by 
  using `Box` (or `Arc/Rc/Vec`). However, to gain control over the memory and prevent the automatic 
  invocation of Drop when the `Box` goes out of scope, developers use transformations like `Box::into_raw`.  
  Subsequently, there needs to be a way to retrieve the pointer and convert it back into a Box, usually by utilizing 
  methods like `from_raw` (If we use `Arc/Rc` is a bit different but let's not get into that 😅).

This library, whenever possible, stores objects in contexts allocated by the OS, such as Driver and Device Extensions.
These objects will be copied into the OS memory, and it's important to note that the original object will be automatically 
dropped when it goes out of scope. However, the copy in OS memory is not dropped. We then provide methods throughout 
the objects to retrieve these contexts.

In cases where we don't have an OS-allocated context, we use `Box::into_raw` to store the pointer and `Box::from_raw` to 
return the context.

### Modules
The library is organized into distinct modules, each offering unique objects and functionality. 
Currently, the following modules are accessible:

| Module    | Information                    | Notes                                                                                                                                                              |
|-----------|--------------------------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| callbacks | Kernel callbacks               | Callback objects + Most kernel callbacks (Ps, Th, Ob, etc..)                                                                                                       |
| common    | Generic objects                | Objects like `OBJECT_ATTRIBUTES`, `EPROCESS`, `ETHREAD`, etc...                                                                                                    |
| io        | I/O related kernel object      | Most `Io` related functions                                                                                                                                        |
| memory    | Memory related kernel objects  | Most `Mm` related functions will be under this module<br/> This module also contains different Allocator impl                                                      |
| registry  | Registry related objects       | Not yet implemented                                                                                                                                                |
| strings   | kernel Strings                 | `STRING` & `ANSI_STRING` not implmeneted.<br/> Split in str & string. str doesn't own the buffer, String owns the bufer                                            |
| sync      | kernel Syncrhonization objects | Most of the objects disccussed in [The State of Synchronization](https://www.osr.com/nt-insider/2015-issue3/the-state-of-synchronization/) (Always a good read 🙂) |

> Please open an issue to discuss if you believe an object is in the wrong module.

### Features
The library offers various optional features. Currently, they include:

| Feature       | Information                                                                                         |
|---------------|-----------------------------------------------------------------------------------------------------|
| `const_new`     | Enables const method (`const_new`) to create objects. (On by default)                               |
| `lock_api`      | Adds implementation of [lock_api](https://docs.rs/lock_api/latest/lock_api/) Mutex and RwLock types |
| `allocator_api` | Enables the usage of the `allocator_api` when allocating memory, providing fallible allocations     |
| `try_non_paged` | Will use NonPaged memory when using TryFrom methods to create Strings                               |
| `unicode_as_vec` | UnicodeString will hold a `Vec<u16>` instead of a `UNICODE_STRING` (Experimental)                   |

### Getting Started
See [examples](examples).

### Contribute
When adding a new wrapper please make sure to use the prefix `Wdu` and create the getters of the actual object using 
either `inner_getters_value` or`inner_getters_ptr`. If required for the object please consider adding the required test 
driver in the [tests](tests) directory. Finally, if the object is being used in some sample driver it would be 
appreciated if an example is added into the [examples](examples) directory.

This repository relies on the [windows-sys](https://docs.rs/crate/windows-sys/latest) crate. If there's any invalid 
prototype for a function/object please refer to the [wdkmetadata](https://github.com/microsoft/wdkmetadata) to 
request a revision of it. On the meantime feel free to define a temporary prototype under the `nt` module so we can 
differentiate prototypes defined in this crate from ones taken from `windows-sys`.

If you have ideas, bug fixes, or new utilities to enhance the library, please consider contributing or opening an issue.

### TODO
- [ ] Properly document the code and host cargo doc.
- [ ] Figure out the best way to mark IRQL for each function (Maybe Trait similar to Send/Sync).
- [ ] Figure out how to capture exceptions in functions like ProbeForXxx.
- [ ] Consider if each module should be feature controlled.
- [ ] Study how we can store/retrieve a Context for each object similar to WDF.
- [ ] Study if we need to provide Singly and Double linked list or using alloc::vec & alloc::collections is enough 
  (I'd encourage the usage of [fallible_vec](https://docs.rs/fallible_vec/latest/fallible_vec/index.html) if 
  possible to avoid OOM conditions)


## Hardware event sample
This sample matches the [event](https://github.com/microsoft/Windows-driver-samples/tree/main/general/event)
sample from the windows-driver-samples. Is intendend to demonstrates different ways a kernel-mode driver can notify 
an application about a hardware event.

This example differs considerably from the MS sample. Important aspects that change is the fact that we rely on a 
`Vec` instead of using a `LIST_ENTRY`. Also the [NOTIFY_RECORD](https://github.com/microsoft/Windows-driver-samples/blob/main/general/event/wdm/event.h#L49)
is defined in a way that fits a Rust design better. We also rely on cloning the pointer of the IRP and KEVENT so we 
can pass a copy to the DPC without having to pass the whole `NotifyRecord`. 

Lastly, this examples shows: 
- How to store an object in the `DeviceExtension` and how we can later retrieve it.
- The usage of `WduSpinLock` without using the `lock_api` feature.

The sample can be tested with the executable from the MS sample. If we enable kernel verbose output then we should
be able to get the following DbgPrint messages:

#### Output using Event
```
// .\event.exe 1 1
INFO  [event] ==> EventCreate
INFO  [event] ==> EventDispatchIoControl
INFO  [event]  tRegisterEventBasedNotification
INFO  [event] ==> CustomTimerDPC
INFO  [event] ==> EventDispatchIoControl
INFO  [event]  tRegisterEventBasedNotification
INFO  [event] ==> CustomTimerDPC
INFO  [event] ==> EventDispatchIoControl
INFO  [event]  tRegisterEventBasedNotification
INFO  [event] ==> CustomTimerDPC
INFO  [event] ==> EventDispatchIoControl
INFO  [event]  tRegisterEventBasedNotification
INFO  [event] ==> CustomTimerDPC
INFO  [event] ==> EventCleanup
INFO  [event] ==> EventClose
INFO  [event] Freeing FsContext
```

#### Output using Pending-IRP
```
// .\event.exe 1 0
INFO  [event] ==> EventCreate
INFO  [event] ==> EventDispatchIoControl
INFO  [event]  RegisterIrpBasedNotification
INFO  [event] ==> CustomTimerDPC
INFO  [event] ==> EventDispatchIoControl
INFO  [event]  RegisterIrpBasedNotification
INFO  [event] ==> CustomTimerDPC
INFO  [event] ==> EventDispatchIoControl
INFO  [event]  RegisterIrpBasedNotification
INFO  [event] ==> CustomTimerDPC
INFO  [event] ==> EventCleanup
INFO  [event] ==> EventClose
INFO  [event] Freeing FsContext
```

#### Output cancel Pending-IRP
```
// .\event.exe 100 0 & pressing Ctrl+C 
INFO  [event] ==> EventCreate
INFO  [event] ==> EventDispatchIoControl
INFO  [event]  RegisterIrpBasedNotification
INFO  [event] ==>EventCancelRoutine irp 0xffffa68ea55aedb0
INFO  [event]  Cancelled timer
INFO  [event]  Cancelled IRP 0xffffa68ea55aedb0
INFO  [event] ==> EventCleanup
INFO  [event] ==> EventClose
INFO  [event] Freeing FsContext
```
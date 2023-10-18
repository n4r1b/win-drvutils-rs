//! Collection of utils to work with MS defined kernel callbacks
use snafu::Snafu;
use windows_sys::Win32::Foundation::NTSTATUS;

pub mod callback;
pub mod cm;
pub mod image;
pub mod ob;
pub mod process;
pub mod thread;

#[derive(Debug, Snafu)]
pub enum WduCallbackError {
    // Generic
    #[snafu(display("Insufficient Resources"))]
    InsufficientResources,
    #[snafu(display("No Callback registered"))]
    NotRegistered,
    #[snafu(display("Unable to register callback. Status {status}"))]
    RegisterError { status: NTSTATUS },
    #[snafu(display("Unable to unregister callback. Status {status}"))]
    UnregisterError { status: NTSTATUS },
    #[snafu(display("NTSTATUS: {status}"))]
    NtStatus { status: NTSTATUS },

    // Callback Object specific
    #[snafu(display("Unable to create callback. Status {status}"))]
    CreateError { status: NTSTATUS },
    #[snafu(display("Unable to open callback. Status {status}"))]
    OpenError { status: NTSTATUS },
}

pub type WduCallbackResult<T> = Result<T, WduCallbackError>;

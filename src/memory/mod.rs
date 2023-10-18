pub mod mdl;
pub mod pool;

use windows_sys::Wdk::Foundation::{NonPagedPool, NonPagedPoolNx, POOL_TYPE};

/// Default tag used by all allocators if client doesn't set one. Set to `ALrs`
const DEFAULT_POOL_TAG: u32 = u32::from_ne_bytes(*b"ALrs");

/// Wrapper over POOL_FLAGS type.
///
/// See <https://learn.microsoft.com/en-us/windows-hardware/drivers/kernel/pool_flags>
#[repr(u64)]
#[derive(Copy, Clone)]
pub enum PoolFlags {
    PoolFlagUninit = 0x2,
    PoolFlagCacheAligned = 0x4,
    PoolFlagNonPaged = 0x40,
    PoolFlagNonPagedExecute = 0x80,
    PoolFlagPaged = 0x100,
}

#[allow(non_upper_case_globals)]
impl From<POOL_TYPE> for PoolFlags {
    fn from(value: POOL_TYPE) -> Self {
        // TODO: This relation is not completely right. PoolFlags allows to bit-wise OR the flags
        match value {
            // NonPagedPoolExecute will also match this arm NonPagedPool == NonPagedPoolExecute
            // Prefer to alloc non-execute memory.
            NonPagedPool | NonPagedPoolNx => PoolFlags::PoolFlagNonPaged,
            _ => PoolFlags::PoolFlagPaged,
        }
    }
}

impl Into<u64> for PoolFlags {
    fn into(self) -> u64 {
        match self {
            PoolFlags::PoolFlagNonPaged => 0x40,
            PoolFlags::PoolFlagPaged => 0x100,
            _ => unreachable!(),
        }
    }
}

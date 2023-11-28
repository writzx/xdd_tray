#![allow(dead_code, non_snake_case, non_camel_case_types, non_upper_case_globals)]

use std::ffi::c_void;
use std::ptr::null_mut;
use std::sync::OnceLock;

#[allow(non_camel_case_types)]
#[must_use]
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MH_STATUS {
    /// Unknown error. Should not be returned.
    MH_UNKNOWN = -1,
    /// Successful.
    MH_OK = 0,
    /// MinHook is already initialized.
    MH_ERROR_ALREADY_INITIALIZED,
    /// MinHook is not initialized yet, or already uninitialized.
    MH_ERROR_NOT_INITIALIZED,
    /// The hook for the specified target function is already created.
    MH_ERROR_ALREADY_CREATED,
    /// The hook for the specified target function is not created yet.
    MH_ERROR_NOT_CREATED,
    /// The hook for the specified target function is already enabled.
    MH_ERROR_ENABLED,
    /// The hook for the specified target function is not enabled yet, or
    /// already disabled.
    MH_ERROR_DISABLED,
    /// The specified pointer is invalid. It points the address of non-allocated
    /// and/or non-executable region.
    MH_ERROR_NOT_EXECUTABLE,
    /// The specified target function cannot be hooked.
    MH_ERROR_UNSUPPORTED_FUNCTION,
    /// Failed to allocate memory.
    MH_ERROR_MEMORY_ALLOC,
    /// Failed to change the memory protection.
    MH_ERROR_MEMORY_PROTECT,
    /// The specified module is not loaded.
    MH_ERROR_MODULE_NOT_FOUND,
    /// The specified function is not found.
    MH_ERROR_FUNCTION_NOT_FOUND,
}


impl From<minhook_sys::MH_STATUS> for MH_STATUS {
    fn from(value: minhook_sys::MH_STATUS) -> Self {
        match value {
            minhook_sys::MH_UNKNOWN => { MH_STATUS::MH_UNKNOWN }
            minhook_sys::MH_OK => { MH_STATUS::MH_OK }
            minhook_sys::MH_ERROR_ALREADY_INITIALIZED => { MH_STATUS::MH_ERROR_ALREADY_INITIALIZED }
            minhook_sys::MH_ERROR_NOT_INITIALIZED => { MH_STATUS::MH_ERROR_NOT_INITIALIZED }
            minhook_sys::MH_ERROR_ALREADY_CREATED => { MH_STATUS::MH_ERROR_ALREADY_CREATED }
            minhook_sys::MH_ERROR_NOT_CREATED => { MH_STATUS::MH_ERROR_NOT_CREATED }
            minhook_sys::MH_ERROR_ENABLED => { MH_STATUS::MH_ERROR_ENABLED }
            minhook_sys::MH_ERROR_DISABLED => { MH_STATUS::MH_ERROR_DISABLED }
            minhook_sys::MH_ERROR_NOT_EXECUTABLE => { MH_STATUS::MH_ERROR_NOT_EXECUTABLE }
            minhook_sys::MH_ERROR_UNSUPPORTED_FUNCTION => { MH_STATUS::MH_ERROR_UNSUPPORTED_FUNCTION }
            minhook_sys::MH_ERROR_MEMORY_ALLOC => { MH_STATUS::MH_ERROR_MEMORY_ALLOC }
            minhook_sys::MH_ERROR_MEMORY_PROTECT => { MH_STATUS::MH_ERROR_MEMORY_PROTECT }
            minhook_sys::MH_ERROR_MODULE_NOT_FOUND => { MH_STATUS::MH_ERROR_MODULE_NOT_FOUND }
            minhook_sys::MH_ERROR_FUNCTION_NOT_FOUND => { MH_STATUS::MH_ERROR_FUNCTION_NOT_FOUND }
            _ => unreachable!()
        }
    }
}

impl MH_STATUS {
    pub fn ok_context(self, _context: &str) -> Result<(), MH_STATUS> {
        if self == MH_STATUS::MH_OK {
            Ok(())
        } else {
            // error!("{context}: {self:?}");
            Err(self)
        }
    }

    pub fn ok(self) -> Result<(), MH_STATUS> {
        if self == MH_STATUS::MH_OK {
            Ok(())
        } else {
            Err(self)
        }
    }
}

type MHCreate = unsafe extern "system" fn(pTarget: *mut c_void, pDetour: *mut c_void, ppOriginal: *mut *mut c_void) -> minhook_sys::MH_STATUS;
type MHOthers = unsafe extern "system" fn(pTarget: *mut c_void) -> minhook_sys::MH_STATUS;

/// holds original address, hook function address, and trampoline address for a given hook.
pub struct mh {
    addr: *mut c_void,
    hook_impl: *mut c_void,
    trampoline: *mut c_void,
}

static MH_CreateHook: OnceLock<MHCreate> = OnceLock::new();
static MH_EnableHook: OnceLock<MHOthers> = OnceLock::new();
static MH_DisableHook: OnceLock<MHOthers> = OnceLock::new();
static MH_RemoveHook: OnceLock<MHOthers> = OnceLock::new();

impl mh {
    pub unsafe fn init(
        create: MHCreate,
        enable: MHOthers,
        disable: MHOthers,
        remove: MHOthers,
    ) -> Result<(), MH_STATUS> {
        if MH_CreateHook.set(create).is_err()
            || MH_EnableHook.set(enable).is_err()
            || MH_DisableHook.set(disable).is_err()
            || MH_RemoveHook.set(remove).is_err() {
            return Err(MH_STATUS::MH_UNKNOWN);
        }

        Ok(())
    }

    pub unsafe fn new(addr: *mut c_void, hook_impl: *mut c_void) -> Result<Self, MH_STATUS> {
        let mut trampoline = null_mut();

        if let Some(create_hook) = MH_CreateHook.get() {
            MH_STATUS::from(create_hook(addr, hook_impl, &mut trampoline)).ok_context("MH_CreateHook")?;
        } else {
            return Err(MH_STATUS::MH_ERROR_NOT_INITIALIZED);
        }

        Ok(Self { addr, hook_impl, trampoline })
    }

    pub fn trampoline(&self) -> *mut c_void {
        self.trampoline
    }

    pub unsafe fn enable(&self) -> Result<(), MH_STATUS> {
        if let Some(enable_hook) = MH_EnableHook.get() {
            MH_STATUS::from(enable_hook(self.addr)).ok_context("MH_EnableHook")
        } else {
            return Err(MH_STATUS::MH_ERROR_NOT_INITIALIZED);
        }
    }

    pub unsafe fn disable(&self) -> Result<(), MH_STATUS> {
        if let Some(disable_hook) = MH_DisableHook.get() {
            MH_STATUS::from(disable_hook(self.addr)).ok_context("MH_DisableHook")
        } else {
            return Err(MH_STATUS::MH_ERROR_NOT_INITIALIZED);
        }
    }

    pub unsafe fn remove(&self) -> Result<(), MH_STATUS> {
        if let Some(remove_hook) = MH_RemoveHook.get() {
            MH_STATUS::from(remove_hook(self.addr)).ok_context("MH_RemoveHook")
        } else {
            return Err(MH_STATUS::MH_ERROR_NOT_INITIALIZED);
        }
    }
}


mod mh;

use std::cell::OnceCell;
use std::ffi::{c_char, c_void, CString};

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

use std::time::{Duration, SystemTime};

use windows::core::{HRESULT, Interface};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::Dxgi::IDXGISwapChain;
use windows::Win32::UI::WindowsAndMessaging::{DefWindowProcW, PostMessageW, SC_MINIMIZE, WM_SYSCOMMAND};

use nexus_rs::raw_structs::{AddonAPI, AddonDefinition, AddonVersion, EAddonFlags, ELogLevel, LPVOID};

static mut PRESENT_HOOK: OnceLock<mh::mh> = OnceLock::new();
static mut TRAMPOLINE: OnceCell<&'static unsafe extern "system" fn(
    this: *mut IDXGISwapChain,
    sync_interval: u32,
    flags: u32) -> HRESULT> = OnceCell::new();
static FRAME_TIME_NS: AtomicU64 = AtomicU64::new(0);
static TIME_OF_LAST_PRESENT_NS: AtomicU64 = AtomicU64::new(0);
macro_rules! log {
    ($a: expr, $b: expr) => {
        log($a, $b, false)
    };
    ($a: expr, $b: expr, $c: expr) => {
        log($a, $b, $c)
    };
}

pub fn mk_str<const N: usize>(str: &[&str; N], f: impl Fn(&[*const c_char; N])) {
    // pass ownership to C code, which should not modify it
    let raw_strs: [*mut c_char; N] = str.map(|v| {
        CString::new(v).unwrap().into_raw()
    });

    let c_strs: [*const c_char; N] = raw_strs.map(|v| {
        v as *const c_char
    });

    // pass to the c function
    f(&c_strs);

    // take ownership back to free
    for raw_str in raw_strs {
        unsafe { let _ = CString::from_raw(raw_str as *mut c_char); }
    }
}


#[no_mangle]
pub extern "C" fn GetAddonDef() -> *mut AddonDefinition {
    static AD: AddonDefinition = AddonDefinition {
        // signature: -0x1337,
        signature: -0x1338,
        apiversion: nexus_rs::raw_structs::NEXUS_API_VERSION,
        // name: b"xddTray\0".as_ptr() as *const c_char,
        name: b"xddTray2\0".as_ptr() as *const c_char,
        version: AddonVersion {
            major: 0,
            minor: 0,
            build: 1,
            revision: 0,
        },
        author: b"writzx\0".as_ptr() as *const c_char,
        description: b"xddTray\0".as_ptr() as *const c_char,
        load,
        unload: Some(unload),
        flags: EAddonFlags::None,
        provider: nexus_rs::raw_structs::EUpdateProvider::None,
        update_link: None,
    };

    &AD as *const _ as _
}

static mut API: OnceLock<&'static AddonAPI> = OnceLock::new();
static WINDOW_HANDLE: OnceLock<HWND> = OnceLock::new();

const DEBUG: bool = true;

pub fn log(a_log_level: ELogLevel, a_str: &str, is_debug: bool) {
    if is_debug && !DEBUG {
        return;
    }

    if let Some(api) = unsafe { API.get() } {
        mk_str(&[a_str], |log_str| {
            unsafe {
                (api.log)(
                    a_log_level,
                    log_str[0],
                );
            }
        });
    }
}

pub unsafe fn hide_window(hwnd: HWND) {
    log!(ELogLevel::TRACE, "hiding main window");
    if PostMessageW(
        hwnd,
        WM_SYSCOMMAND,
        WPARAM(SC_MINIMIZE as _),
        LPARAM(0 as _),
    ).is_err() {
        log!(ELogLevel::WARNING, "could not hide main window");
    }
}

unsafe extern "C" fn trayize_bind_callback(_: *const i8) {
    if let Some(h) = WINDOW_HANDLE.get() {
        hide_window(*h);
    } else {
        log!(ELogLevel::CRITICAL, "unable to get window handle");
    }
}

unsafe extern "C" fn window_handle_callback(hwnd: *mut c_void) {
    log!(ELogLevel::TRACE, "received window handle callback");
    let Some(_) = WINDOW_HANDLE.get() else {
        log!(ELogLevel::TRACE, "setting global window handle");
        if WINDOW_HANDLE.set(HWND(hwnd as isize)).is_err() {
            log!(ELogLevel::CRITICAL, "unable to set window handle");
            return;
        }

        log!(ELogLevel::TRACE, "unregistering wnd_proc");
        if let Some(api) = API.get() {
            (api.unregister_wnd_proc)(window_procedure);
        }
        return;
    };
}

unsafe extern "C" fn new_present_function(this: *mut IDXGISwapChain, sync_interval: u32, flags: u32) -> HRESULT {

    log!(ELogLevel::TRACE, "hello from present");

    let trampoline = TRAMPOLINE.get().unwrap();
    trampoline(this, sync_interval, flags)
}

unsafe extern "C" fn load(a_api: *mut AddonAPI) {
    if API.set(&*a_api).is_err() {
        // log!(ELogLevel::CRITICAL, "unable to set api context");
        panic!("unable to set api context");
        // return;
    }

    log!(ELogLevel::TRACE, "loaded addon");

    log!(ELogLevel::TRACE, "set global api context");

    if let Some(api) = API.get() {
        (api.register_keybind_with_string)(
            "KB_TRAYIZE\0" as *const _ as _,
            trayize_bind_callback,
            "ALT+Q\0" as *const _ as _,
        );

        (api.register_wnd_proc)(window_procedure);

        (api.subscribe_event)("WINDOW_HANDLE_RECEIVED\0" as *const _ as _, window_handle_callback);


        if mh::mh::init(
            api.create_hook,
            api.enable_hook,
            api.disable_hook,
            api.remove_hook,
        ).is_err() {
            log!(ELogLevel::CRITICAL, "unable to init hooks");
            return;
        }

        let swap_chain = IDXGISwapChain::from_raw(api.swap_chain as *mut c_void);

        log!(ELogLevel::TRACE, format!("SWAP CHAIN {:p}", swap_chain.vtable()).as_str());

        let present_function = swap_chain.vtable().Present as *mut LPVOID;

        log!(ELogLevel::TRACE, format!("PRESENT {:p}", present_function as *mut LPVOID).as_str());
        log!(ELogLevel::TRACE, format!("NEW PRESENT {:p}", new_present_function as *mut LPVOID).as_str());


        let new_hook = mh::mh::new(present_function as *mut _, new_present_function as *mut _);

        let Ok(hook_present) = new_hook else {
            log!(ELogLevel::CRITICAL, format!("unable to create hook: {:?}", new_hook.err().unwrap()).as_str());
            return;
        };
        log!(ELogLevel::TRACE, "hooked swap chain present");

        if TRAMPOLINE.set(std::mem::transmute(hook_present.trampoline())).is_err() {
            log!(ELogLevel::CRITICAL, "unable to set trampoline");
            return;
        }
        log!(ELogLevel::TRACE, "trampoline was set");

        let enabled = hook_present.enable();

        let Ok(()) = enabled else {
            log!(ELogLevel::CRITICAL, format!("unable to enable hook: {:?}", enabled.err().unwrap()).as_str());
            return;
        };
        log!(ELogLevel::TRACE, "swap chain present hook enabled");

        if PRESENT_HOOK.set(hook_present).is_err() {
            log!(ELogLevel::CRITICAL, "unable to set global hook");
            return;
        }
        log!(ELogLevel::TRACE, "global hook was set");
    }
}

unsafe extern "C" fn window_procedure(
    h_wnd: *mut c_void,
    u_msg: u32,
    w_param: usize,
    l_param: isize,
) -> u32 {
    if let Some(api) = API.get() {
        log!(ELogLevel::TRACE, "raising window handle event");
        (api.raise_event)("WINDOW_HANDLE_RECEIVED\0" as *const _ as _, h_wnd);
    }

    &DefWindowProcW(
        HWND(h_wnd as isize),
        u_msg,
        WPARAM(w_param as _),
        LPARAM(l_param as _),
    ) as *const _ as _
}

unsafe extern "C" fn unload() {
    log!(ELogLevel::TRACE, "unloading the addon");
    if let Some(api) = API.get() {
        (api.unregister_keybind)("KB_TRAYIZE\0" as *const _ as _);

        if let (Some(_), Some(hook_present)) = (TRAMPOLINE.get(), PRESENT_HOOK.get()) {
            let disabled = hook_present.disable();

            let Ok(()) = disabled else {
                log!(ELogLevel::CRITICAL, format!("unable to disable hook: {:?}", disabled.err().unwrap()).as_str());
                return;
            };

            let removed = hook_present.remove();

            let Ok(()) = removed else {
                log!(ELogLevel::CRITICAL, format!("unable to remove hook: {:?}", removed.err().unwrap()).as_str());
                return;
            };
        }
    }
}

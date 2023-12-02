use std::ffi::{c_char, c_void, CString};

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{OnceLock};
use std::time::{Duration, SystemTime};

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, SC_MINIMIZE, WM_SYSCOMMAND, SC_MAXIMIZE, SC_RESTORE};

use nexus_rs::raw_structs::{AddonAPI, AddonDefinition, AddonVersion, EAddonFlags, ELogLevel, ERenderType};

static mut API: OnceLock<&'static AddonAPI> = OnceLock::new();

static mut WINDOW_HANDLE: OnceLock<&'static HWND> = OnceLock::new();

const DEBUG: bool = true;
const HIDDEN_FPS_LIMIT: u32 = 3;

static FRAME_TIME_NS: AtomicU64 = AtomicU64::new(0);
static TIME_OF_LAST_PRESENT_NS: AtomicU64 = AtomicU64::new(0);

macro_rules! log {
    ($a: expr, $b: expr) => {
        log($a, $b, ($a as u8) >= (ELogLevel::DEBUG as u8))
    };
    ($a: expr, $b: expr, $c: expr) => {
        log($a, $b, $c)
    };
}

macro_rules! fps_to_ns {
    ($fps: expr) => {
        match $fps {
            0 => 0,
            _ => ((1f64 / $fps as f64)
                * 1000f64
                * 1000f64
                * 1000f64) as u64
        }
    };
}

macro_rules! set_fps_limit {
    ($fps: expr) => {
        FRAME_TIME_NS.store(fps_to_ns!($fps), Ordering::Relaxed);
    };
}

pub fn use_str<const N: usize>(str: &[&str; N], f: impl Fn(&[*const c_char; N])) {
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

pub fn log(a_log_level: ELogLevel, a_str: &str, is_debug: bool) {
    if is_debug && !DEBUG {
        return;
    }

    if let Some(api) = unsafe { API.get() } {
        use_str(&[a_str], |log_str| {
            unsafe {
                (api.log)(
                    a_log_level,
                    log_str[0],
                );
            }
        });
    }
}

#[no_mangle]
pub extern "C" fn GetAddonDef() -> *mut AddonDefinition {
    static AD: AddonDefinition = AddonDefinition {
        signature: -0x1337,
        apiversion: nexus_rs::raw_structs::NEXUS_API_VERSION,
        name: b"xddTray\0".as_ptr() as *const c_char,
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

fn enable_limiter() {
    log!(ELogLevel::TRACE, "enabling fps limit");
    set_fps_limit!(HIDDEN_FPS_LIMIT);
}

fn disable_limiter() {
    log!(ELogLevel::TRACE, "disabling fps limit");
    set_fps_limit!(0); // disable
}

pub fn minimize(h_wnd: HWND) {
    log!(ELogLevel::TRACE, "minimizing main window");
    if unsafe {
        PostMessageW(
            h_wnd,
            WM_SYSCOMMAND,
            WPARAM(SC_MINIMIZE as _),
            LPARAM(0 as _),
        ).is_err()
    } {
        log!(ELogLevel::WARNING, "could not minimize main window");
        return;
    }
}

pub fn minimize_with_limiter(h_wnd: HWND) {
    minimize(h_wnd);
    enable_limiter();
}

unsafe extern "C" fn window_procedure(
    h_wnd: *mut c_void,
    u_msg: u32,
    u_param: usize,
    _: isize,
) -> u32 {
    if let Some(api) = API.get() {
        match WINDOW_HANDLE.get() {
            Some(_) => {}
            _ => {
                log!(ELogLevel::TRACE, "raising window handle event");
                (api.raise_event)("WINDOW_HANDLE_RECEIVED\0" as *const _ as _, h_wnd);
            }
        }
    }
    match u_msg {
        WM_SYSCOMMAND => {
            match u_param as u32 {
                // SC_MINIMIZE => {
                //     // set_fps_limit!();
                // }
                SC_MAXIMIZE | SC_RESTORE => {
                    disable_limiter();
                }
                _ => {}
            }
        }
        _ => {}
    }

    1 // we are not handling it at all
}

unsafe extern "C" fn trayize(_: *const i8) {
    if let Some(h_wnd) = WINDOW_HANDLE.get() {
        minimize_with_limiter(**h_wnd);
    } else {
        log!(ELogLevel::CRITICAL, "failed to get window handle");
    }
}

unsafe extern "C" fn window_handle_callback(h_wnd: *mut c_void) {
    log!(ELogLevel::TRACE, "received window handle callback");
    let Some(_) = WINDOW_HANDLE.get() else {
        log!(ELogLevel::TRACE, "setting global window handle");
        if WINDOW_HANDLE.set(Box::leak(Box::new(HWND(h_wnd as isize)))).is_err() {
            log!(ELogLevel::CRITICAL, "failed to set window handle");
            return;
        }

        if let Some(api) = API.get() {
            log!(ELogLevel::TRACE, "unsubscribing from window handle event");
            (api.unsubscribe_event)("WINDOW_HANDLE_RECEIVED\0" as *const _ as _, window_handle_callback);
        }
        return;
    };
}

unsafe extern "C" fn limiter() {
    let frame_time_ns = FRAME_TIME_NS.load(Ordering::Relaxed);
    if frame_time_ns != 0 {
        let current_time_ns = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        let time_between_last_present_call =
            current_time_ns - TIME_OF_LAST_PRESENT_NS.load(Ordering::Relaxed);
        if time_between_last_present_call < frame_time_ns {
            std::thread::sleep(
                Duration::from_nanos(
                    frame_time_ns - time_between_last_present_call
                )
            );
        }

        TIME_OF_LAST_PRESENT_NS.store(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64,
            Ordering::Relaxed,
        );
    }
}

unsafe extern "C" fn load(a_api: *mut AddonAPI) {
    if API.set(&*a_api).is_err() {
        // log!(ELogLevel::CRITICAL, "failed to set api context");
        panic!("failed to set api context");
        // return;
    }

    log!(ELogLevel::TRACE, "loaded addon");

    log!(ELogLevel::TRACE, "set global api context");

    if let Some(api) = API.get() {
        (api.register_keybind_with_string)(
            "KB_TRAYIZE\0" as *const _ as _,
            trayize,
            "ALT+Q\0" as *const _ as _,
        );

        (api.register_wnd_proc)(window_procedure);

        (api.subscribe_event)("WINDOW_HANDLE_RECEIVED\0" as *const _ as _, window_handle_callback);

        (api.register_render)(ERenderType::PostRender, limiter);
    }
}

unsafe extern "C" fn unload() {
    log!(ELogLevel::TRACE, "unloading the addon");
    if let Some(api) = API.get() {
        (api.unregister_keybind)("KB_TRAYIZE\0" as *const _ as _);

        (api.unregister_render)(limiter);
    }
    //
    // API.take();
    // WINDOW_HANDLE.take();
}

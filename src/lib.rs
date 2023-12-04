mod wm;

use crate::wm::{WindowManager, WM_WINDOW_STATE};

use std::ffi::{c_char, c_void, CString};

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{OnceLock};
use std::time::{Duration, SystemTime};

use nexus_rs::raw_structs::{AddonAPI, AddonDefinition, AddonVersion, EAddonFlags, ELogLevel, ERenderType};

static mut API: OnceLock<&'static AddonAPI> = OnceLock::new();

static mut WM: OnceLock<&mut WindowManager> = OnceLock::new();

const DEBUG: bool = true;

static FRAME_TIME_NS: AtomicU64 = AtomicU64::new(0);
static TIME_OF_LAST_PRESENT_NS: AtomicU64 = AtomicU64::new(0);

const HIDDEN_FPS_LIMIT: u32 = 3;
const INACTIVE_FPS_LIMIT: u32 = 15;

/*
/// default enabled
static HOTKEY_ENABLE: AtomicBool = AtomicBool::new(true);

/// default enabled
static HIDDEN_LIMIT_ENABLE: AtomicBool = AtomicBool::new(true);

/// default enabled
static INACTIVE_LIMIT_ENABLE: AtomicBool = AtomicBool::new(true);

static HIDDEN_FPS: AtomicU16 = AtomicU16::new(3);
static INACTIVE_FPS: AtomicU16 = AtomicU16::new(15);
*/

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

unsafe extern "C" fn window_procedure(
    h_wnd: *mut c_void,
    u_msg: u32,
    w_param: usize,
    l_param: isize,
) -> u32 {
    if let Some(api) = API.get() {
        if let Some(&mut ref mut wm) = WM.get_mut() {
            if wm.wnd_proc(u_msg, w_param, l_param) {
                return 0;
            }
        } else {
            log!(ELogLevel::TRACE, "raising window handle event");
            (api.raise_event)("WINDOW_HANDLE_RECEIVED\0" as *const _ as _, h_wnd);
        }
    }

    1
}

unsafe extern "C" fn trayize(_: *const i8) {
    if let Some(winman) = WM.get() {
        log!(ELogLevel::TRACE, "minimizing main window");
        if winman.minimize().is_err() {
            log!(ELogLevel::WARNING, "could not minimize main window");
        }

        log!(ELogLevel::TRACE, "enabling fps limit");
        set_fps_limit!(HIDDEN_FPS_LIMIT);
    } else {
        log!(ELogLevel::CRITICAL, "failed to get window manager");
    }
}

unsafe extern "C" fn window_handle_callback(h_wnd: *mut c_void) {
    log!(ELogLevel::TRACE, "received window handle callback");
    match WM.get() {
        Some(_) => {}
        _ => {
            log!(ELogLevel::TRACE, "setting global window handle");
            let mut wm = WindowManager::new(h_wnd);
            wm.on_state_change = Some(window_state_change);

            if WM.set(Box::leak(Box::new(wm))).is_err() {
                log!(ELogLevel::CRITICAL, "failed to set window handle");
                return;
            }

            if let Some(api) = API.get() {
                log!(ELogLevel::TRACE, "unsubscribing from window handle event");
                (api.unsubscribe_event)("WINDOW_HANDLE_RECEIVED\0" as *const _ as _, window_handle_callback);
            }
        }
    }
}

fn window_state_change(state: WM_WINDOW_STATE) {
    match state {
        WM_WINDOW_STATE::MINIMIZED => {
            log!(ELogLevel::TRACE, "enabling hidden fps limit");
            set_fps_limit!(HIDDEN_FPS_LIMIT);
        }
        WM_WINDOW_STATE::INACTIVE => {
            log!(ELogLevel::TRACE, "enabling inactive fps limit");
            set_fps_limit!(INACTIVE_FPS_LIMIT);
        }
        WM_WINDOW_STATE::RESTORED | WM_WINDOW_STATE::MAXIMIZED | WM_WINDOW_STATE::ACTIVE => {
            log!(ELogLevel::TRACE, "disabling fps limit");
            set_fps_limit!(0); // disable
        }
        _ => {}
    }
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

        (api.unregister_wnd_proc)(window_procedure);
    }
}

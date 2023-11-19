use std::ffi::{c_char};
use std::mem::MaybeUninit;
use nexus_rs::raw_structs::{AddonAPI, AddonDefinition, AddonVersion, EAddonFlags, ELogLevel};
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowTextW, PostMessageW, SC_MINIMIZE, WM_SYSCOMMAND};

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

static mut API: MaybeUninit<&'static AddonAPI> = MaybeUninit::uninit();

unsafe extern "system" fn enum_window(window: HWND, _: LPARAM) -> BOOL {
    let mut text: [u16; 512] = [0; 512];
    let len = GetWindowTextW(window, &mut text);
    let text = String::from_utf16_lossy(&text[..len as usize]);

    if text.as_str() == "Guild Wars 2" {
        PostMessageW(
            window,
            WM_SYSCOMMAND,
            WPARAM(SC_MINIMIZE as _),
            LPARAM(0 as _),
        ).ok();
    }

    true.into()
}

unsafe extern "C" fn load(a_api: *mut AddonAPI) {
    let api = &*a_api;
    API.write(&api);

    unsafe extern "C" fn shortcut_callback(_: *const i8) {
        (API.assume_init().log)(
            ELogLevel::INFO,
            b"testing log not found\0".as_ptr() as _,
        );

        EnumWindows(Some(enum_window), LPARAM(0));
    }

    (api.register_keybind_with_string)("KB_TRAYIZE\0" as *const _ as _, shortcut_callback, "F3\0" as *const _ as _);
}

unsafe extern "C" fn unload() {
    (API.assume_init().unregister_keybind)("KB_TRAYIZE\0" as *const _ as _);
}

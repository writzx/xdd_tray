#![allow(non_camel_case_types, dead_code, unused_macros)]

use std::time::SystemTime;
use windows::{
    core::{h},
    Win32::{
        Foundation::{
            HWND,
            LPARAM,
            WPARAM,
        },
        UI::{
            Shell::{
                NIF_ICON,
                NIF_INFO,
                NIF_MESSAGE,
                NIF_SHOWTIP,
                NIF_TIP,
                NIM_ADD,
                NIM_DELETE,
                NIM_SETVERSION,
                NIN_SELECT,
                NOTIFYICON_VERSION_4,
                NOTIFYICONDATAW,
                Shell_NotifyIconW,
            },
            WindowsAndMessaging::{
                GetClassLongW,
                WA_INACTIVE,
                WA_ACTIVE,
                WA_CLICKACTIVE,
                WM_ACTIVATE,
                SC_RESTORE,
                SC_MAXIMIZE,
                WM_SYSCOMMAND,
                SC_MINIMIZE,
                PostMessageW,
                WM_SHOWWINDOW,
                GCL_HICON,
                HICON,
                WM_APP,
                SW_HIDE,
                SW_SHOW,
                ShowWindowAsync as ShowWindow,
            },
        },
    },
};

pub(crate) const WM_NOTIFY_CALLBACK: u32 = WM_APP + 0x69;
pub(crate) const WM_XDD_MESSAGE: u32 = 1337;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum WM_WINDOW_STATE {
    DEFAULT = 0,
    MINIMIZED = 1,
    RESTORED = 2,
    MAXIMIZED = 3,
    HIDDEN = 4,
    SHOWN = 5,
    ACTIVE = 6,
    INACTIVE = 7,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum WM_ERROR {
    WM_ERROR_MINIMIZE = 1,
    WM_ERROR_RESTORE = 2,
    WM_ERROR_MAXIMIZE = 3,
    WM_ERROR_ADD_NOTIFY_ICON = 4,
    WM_ERROR_DEL_NOTIFY_ICON = 5,
    WM_ERROR_SHOW_WINDOW = 6,
    WM_ERROR_HIDE_WINDOW = 7,
}

pub(crate) enum WM_ACTION {
    WM_DEFAULT = 0,
    WM_TRAY_CLICK = 1,
}

pub(crate) struct WindowManager {
    pub handle: HWND,
    pub state: WM_WINDOW_STATE,

    notify_id: NOTIFYICONDATAW,
    pub notify_visible: bool,

    pub window_visible: bool,

    pub on_state_change: Option<fn(_: WM_WINDOW_STATE, _: bool)>,
    pub on_tray_action: Option<fn(_: WM_ACTION) -> bool>,
}

macro_rules! sz_string {
    ($str: expr, $len: expr) => {{
        let val = h!($str);
        let mut ret_val: [u16; $len] = [0u16; $len];

        ret_val[..val.as_wide().len()].copy_from_slice(val.as_wide());

        ret_val as [u16; $len]
    }};
}

macro_rules! hi_word {
    ($word: expr) => {{($word >> 16) & 0xffff}};
}

macro_rules! lo_word {
    ($word: expr) => {{$word & 0xffff}};
}

impl WindowManager {
    pub(crate) fn new(hwnd: nexus_rs::raw_structs::HWND) -> Self {
        let handle = HWND(hwnd as isize);
        let mut notify_id = NOTIFYICONDATAW::default();

        notify_id.hWnd = handle;

        notify_id.uID = SystemTime::now().elapsed().expect("wtf??").as_millis() as u32;
        notify_id.uFlags = NIF_ICON | NIF_TIP | NIF_SHOWTIP | NIF_MESSAGE | NIF_INFO;
        notify_id.hIcon = unsafe { HICON(GetClassLongW(handle, GCL_HICON) as isize) };
        notify_id.uCallbackMessage = WM_NOTIFY_CALLBACK;
        notify_id.Anonymous.uVersion = NOTIFYICON_VERSION_4;

        notify_id.szTip = sz_string!("Show Guild Wars 2 Window", 128);

        // todo fix
        // notify_id.szInfoTitle = sz_string!("Guild Wars 2", 64);
        // notify_id.szInfo = sz_string!("Restore Guild Wars 2 Window", 256);

        Self {
            handle,
            state: WM_WINDOW_STATE::DEFAULT,
            notify_id,
            notify_visible: false,
            window_visible: true,
            on_state_change: None,
            on_tray_action: None,
        }
    }

    pub(crate) fn minimize(&self) -> Result<(), WM_ERROR> {
        let post_message_res = unsafe {
            PostMessageW(
                self.handle,
                WM_SYSCOMMAND,
                WPARAM(SC_MINIMIZE as _),
                LPARAM(WM_XDD_MESSAGE as _),
            )
        };

        post_message_res.or_else(|_| { Err(WM_ERROR::WM_ERROR_MINIMIZE) })
    }

    pub(crate) fn maximize(&self) -> Result<(), WM_ERROR> {
        let post_message_res = unsafe {
            PostMessageW(
                self.handle,
                WM_SYSCOMMAND,
                WPARAM(SC_MAXIMIZE as _),
                LPARAM(WM_XDD_MESSAGE as _),
            )
        };

        post_message_res.or_else(|_| { Err(WM_ERROR::WM_ERROR_MAXIMIZE) })
    }

    pub(crate) fn restore(&self) -> Result<(), WM_ERROR> {
        let post_message_res = unsafe {
            PostMessageW(
                self.handle,
                WM_SYSCOMMAND,
                WPARAM(SC_RESTORE as _),
                LPARAM(WM_XDD_MESSAGE as _),
            )
        };

        post_message_res.or_else(|_| { Err(WM_ERROR::WM_ERROR_RESTORE) })
    }

    pub(crate) fn trayize(&mut self) -> Result<(), WM_ERROR> {
        if self.notify_visible { return Ok(()); }

        let notify_add = unsafe {
            Shell_NotifyIconW(NIM_ADD, &mut self.notify_id)
        };

        let notify_ver = unsafe {
            Shell_NotifyIconW(NIM_SETVERSION, &mut self.notify_id)
        };

        if notify_add.as_bool() && notify_ver.as_bool() {
            self.notify_visible = true;
            Ok(())
        } else {
            Err(WM_ERROR::WM_ERROR_ADD_NOTIFY_ICON)
        }
    }

    pub(crate) fn untrayize(&mut self) -> Result<(), WM_ERROR> {
        if !self.notify_visible { return Ok(()); }

        let notify_del = unsafe {
            Shell_NotifyIconW(NIM_DELETE, &mut self.notify_id)
        };

        if notify_del.as_bool() {
            self.notify_visible = false;
            Ok(())
        } else {
            Err(WM_ERROR::WM_ERROR_DEL_NOTIFY_ICON)
        }
    }

    pub(crate) fn show(&mut self) -> Result<(), WM_ERROR> {
        if self.window_visible { return Ok(()); }
        let show_win_res = unsafe {
            ShowWindow(
                self.handle,
                SW_SHOW,
            )
        };

        if show_win_res.as_bool() {
            self.window_visible = true;
            Ok(())
        } else {
            Err(WM_ERROR::WM_ERROR_SHOW_WINDOW)
        }
    }

    pub(crate) fn hide(&mut self) -> Result<(), WM_ERROR> {
        if !self.window_visible { return Ok(()); }
        let hide_win_res = unsafe {
            ShowWindow(
                self.handle,
                SW_HIDE,
            )
        };

        if hide_win_res.as_bool() {
            self.window_visible = false;
            Ok(())
        } else {
            Err(WM_ERROR::WM_ERROR_HIDE_WINDOW)
        }
    }

    fn state(&mut self, value: WM_WINDOW_STATE, is_self: bool) {
        if value == self.state { return; }
        self.state = value;

        if let Some(on_state) = self.on_state_change {
            on_state(value, is_self);
        }
    }

    pub(crate) fn wnd_proc(&mut self, u_msg: u32, w_param: usize, l_param: isize) -> bool {
        match u_msg {
            WM_SYSCOMMAND => {
                match w_param as u32 {
                    SC_MAXIMIZE => self.state(WM_WINDOW_STATE::MAXIMIZED, l_param as u32 == WM_XDD_MESSAGE),
                    SC_RESTORE => self.state(WM_WINDOW_STATE::RESTORED, l_param as u32 == WM_XDD_MESSAGE),
                    SC_MINIMIZE => self.state(WM_WINDOW_STATE::MINIMIZED, l_param as u32 == WM_XDD_MESSAGE),
                    _ => {}
                }
            }
            WM_ACTIVATE => {
                match w_param as u32 {
                    WA_CLICKACTIVE | WA_ACTIVE => self.state(WM_WINDOW_STATE::ACTIVE, l_param as u32 == WM_XDD_MESSAGE),
                    WA_INACTIVE => self.state(WM_WINDOW_STATE::INACTIVE, l_param as u32 == WM_XDD_MESSAGE),
                    _ => {}
                }
            }
            WM_SHOWWINDOW => {
                if l_param == 0 { // only handle calls from ShowWindow (for now)
                    match w_param as u32 {
                        0 => self.state(WM_WINDOW_STATE::HIDDEN, true),
                        _ => self.state(WM_WINDOW_STATE::SHOWN, true)
                    }
                }
            }
            WM_NOTIFY_CALLBACK => {
                match lo_word!(l_param as u32) {
                    NIN_SELECT => {
                        if let Some(on_tray_notify) = self.on_tray_action {
                            return on_tray_notify(WM_ACTION::WM_TRAY_CLICK);
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        false // we just record the state changes, we don't handle anything for now
    }
}

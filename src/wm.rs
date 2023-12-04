#![allow(non_camel_case_types, dead_code)]

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, SC_MINIMIZE, WM_SYSCOMMAND, SC_MAXIMIZE, SC_RESTORE, WM_ACTIVATE, WA_CLICKACTIVE, WA_ACTIVE, WA_INACTIVE};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum WM_WINDOW_STATE {
    DEFAULT = 0,
    MINIMIZED = 1,
    RESTORED = 2,
    MAXIMIZED = 3,
    ACTIVE = 4,
    INACTIVE = 5,
}

pub(crate) enum WM_ERROR {
    WM_ERROR_MINIMIZE
}

pub(crate) struct WindowManager {
    pub handle: HWND,
    pub state: WM_WINDOW_STATE,

    pub on_state_change: Option<fn(_: WM_WINDOW_STATE)>,
}

impl WindowManager {
    pub(crate) fn new(hwnd: nexus_rs::raw_structs::HWND) -> Self {
        Self {
            handle: HWND(hwnd as isize),
            state: WM_WINDOW_STATE::DEFAULT,
            on_state_change: None,
        }
    }

    pub(crate) fn minimize(&self) -> Result<(), WM_ERROR> {
        let post_message_res = unsafe {
            PostMessageW(
                self.handle,
                WM_SYSCOMMAND,
                WPARAM(SC_MINIMIZE as _),
                LPARAM(0 as _),
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
                LPARAM(0 as _),
            )
        };

        post_message_res.or_else(|_| { Err(WM_ERROR::WM_ERROR_MINIMIZE) })
    }

    pub(crate) fn restore(&self) -> Result<(), WM_ERROR> {
        let post_message_res = unsafe {
            PostMessageW(
                self.handle,
                WM_SYSCOMMAND,
                WPARAM(SC_RESTORE as _),
                LPARAM(0 as _),
            )
        };

        post_message_res.or_else(|_| { Err(WM_ERROR::WM_ERROR_MINIMIZE) })
    }

    fn state(&mut self, value: WM_WINDOW_STATE) {
        self.state = value;

        if let Some(on_state) = self.on_state_change {
            on_state(value);
        }
    }

    pub(crate) fn wnd_proc(&mut self, u_msg: u32, w_param: usize, _: isize) -> bool {
        match u_msg {
            WM_SYSCOMMAND => {
                match w_param as u32 {
                    SC_MAXIMIZE => self.state(WM_WINDOW_STATE::MAXIMIZED),
                    SC_RESTORE => self.state(WM_WINDOW_STATE::RESTORED),
                    SC_MINIMIZE => self.state(WM_WINDOW_STATE::MINIMIZED),
                    _ => {}
                }
            }
            WM_ACTIVATE => {
                match w_param as u32 {
                    WA_CLICKACTIVE | WA_ACTIVE => self.state(WM_WINDOW_STATE::ACTIVE),
                    WA_INACTIVE => self.state(WM_WINDOW_STATE::INACTIVE),
                    _ => {}
                }
            }
            _ => {}
        }

        return false; // we just record the state changes, we don't handle anything for now
    }
}
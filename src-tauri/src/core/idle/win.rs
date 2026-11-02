//! Windows idle source: `GetLastInputInfo`, XInput gamepads and the input desktop.

use std::time::Instant;

use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows_sys::Win32::System::StationsAndDesktops::{
    CloseDesktop, DESKTOP_SWITCHDESKTOP, OpenInputDesktop,
};
use windows_sys::Win32::System::SystemInformation::GetTickCount64;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

use super::{IdleReading, IdleSource, ticks_since};

const XINPUT_SLOTS: usize = 4;
const XINPUT_DLLS: [&str; 3] = ["xinput1_4.dll", "xinput1_3.dll", "xinput9_1_0.dll"];

/// `XINPUT_STATE`: a packet number followed by the 12-byte gamepad block.
#[repr(C)]
#[derive(Default)]
struct XInputState {
    packet_number: u32,
    gamepad: [u8; 12],
}

type XInputGetState = unsafe extern "system" fn(u32, *mut XInputState) -> u32;

pub struct WinIdleSource {
    xinput: Option<XInputGetState>,
    xinput_loaded: bool,
    packets: [Option<u32>; XINPUT_SLOTS],
    pad_input_at: Option<Instant>,
}

impl WinIdleSource {
    pub fn new() -> Self {
        Self {
            xinput: None,
            xinput_loaded: false,
            packets: [None; XINPUT_SLOTS],
            pad_input_at: None,
        }
    }

    /// A changed packet number on a connected slot counts as fresh input. The first sample of
    /// a slot only sets the baseline.
    fn poll_pads(&mut self) {
        if !self.xinput_loaded {
            self.xinput_loaded = true;
            self.xinput = load_xinput();
        }
        let Some(get_state) = self.xinput else {
            return;
        };
        for (slot, last) in self.packets.iter_mut().enumerate() {
            let mut state = XInputState::default();
            // SAFETY: `get_state` is XInputGetState and `state` matches XINPUT_STATE.
            let rc = unsafe { get_state(slot as u32, &mut state) };
            if rc != 0 {
                *last = None;
                continue;
            }
            if last.is_some_and(|prev| prev != state.packet_number) {
                self.pad_input_at = Some(Instant::now());
            }
            *last = Some(state.packet_number);
        }
    }
}

impl Default for WinIdleSource {
    fn default() -> Self {
        Self::new()
    }
}

impl IdleSource for WinIdleSource {
    fn read(&mut self) -> IdleReading {
        self.poll_pads();
        let keyboard = last_input_idle_secs();
        let pad = self.pad_input_at.map(|t| t.elapsed().as_secs() as i64);
        let idle_secs = match (keyboard, pad) {
            (Some(k), Some(p)) => Some(k.min(p)),
            (k, p) => k.or(p),
        };
        IdleReading {
            idle_secs,
            locked: input_desktop_unavailable(),
        }
    }

    fn reset(&mut self) {
        self.packets = [None; XINPUT_SLOTS];
        self.pad_input_at = None;
    }
}

fn load_xinput() -> Option<XInputGetState> {
    for name in XINPUT_DLLS {
        let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: `wide` is NUL terminated and outlives the call.
        let module = unsafe { LoadLibraryW(wide.as_ptr()) };
        if module.is_null() {
            continue;
        }
        // SAFETY: the module handle is valid and the name is a NUL terminated C string.
        if let Some(proc) = unsafe { GetProcAddress(module, c"XInputGetState".as_ptr().cast()) } {
            // SAFETY: XInputGetState has exactly the signature of `XInputGetState`.
            return Some(unsafe {
                std::mem::transmute::<unsafe extern "system" fn() -> isize, XInputGetState>(proc)
            });
        }
    }
    None
}

fn last_input_idle_secs() -> Option<i64> {
    let mut info = LASTINPUTINFO {
        cbSize: size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    // SAFETY: `info` is initialised with its size as the API requires.
    if unsafe { GetLastInputInfo(&mut info) } == 0 {
        return None;
    }
    // SAFETY: no arguments.
    let now = unsafe { GetTickCount64() };
    Some((ticks_since(now, info.dwTime) / 1000) as i64)
}

/// The input desktop cannot be opened while the workstation is locked (or a secure desktop,
/// such as a UAC prompt, is showing).
fn input_desktop_unavailable() -> bool {
    // SAFETY: the handle is closed right after the check.
    unsafe {
        let desktop = OpenInputDesktop(0, 0, DESKTOP_SWITCHDESKTOP);
        if desktop.is_null() {
            return true;
        }
        CloseDesktop(desktop);
        false
    }
}

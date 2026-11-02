//! Real process enumeration: `sysinfo` for the process list, Win32 for the foreground window.

use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

use super::{ProcessSource, Snapshot};
use crate::core::db::repo::normalize_path;
use crate::core::importers::registry::{WinRegistry, steam_running_app_id};

pub struct SysinfoProcessSource {
    system: System,
}

impl SysinfoProcessSource {
    pub fn new() -> Result<Self, sysinfo::Error> {
        Ok(Self {
            system: System::new()?,
        })
    }
}

impl ProcessSource for SysinfoProcessSource {
    fn snapshot(&mut self) -> Snapshot {
        // Only the exe path is needed; skip CPU, memory, command line and the rest.
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
        );
        let mut processes: Vec<String> = self
            .system
            .processes()
            .values()
            .filter_map(|p| p.exe())
            .map(|path| normalize_path(&path.to_string_lossy()))
            .collect();
        processes.sort_unstable();
        processes.dedup();
        let mut snapshot = Snapshot {
            processes,
            foreground: foreground_process_path(),
            steam_running_app_id: None,
        };
        // The registry value goes stale when Steam exits, so only trust it while Steam runs.
        if snapshot.steam_running() {
            snapshot.steam_running_app_id = steam_running_app_id(&WinRegistry);
        }
        snapshot
    }
}

/// Normalised image path of the process owning the foreground window, if readable.
fn foreground_process_path() -> Option<String> {
    // SAFETY: plain Win32 calls; the process handle is closed on every path and the buffer
    // outlives the call that fills it.
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return None;
        }
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }
        let mut buf = vec![0u16; 32_768];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut len);
        CloseHandle(handle);
        if ok == 0 {
            return None;
        }
        Some(normalize_path(&String::from_utf16_lossy(
            &buf[..len as usize],
        )))
    }
}

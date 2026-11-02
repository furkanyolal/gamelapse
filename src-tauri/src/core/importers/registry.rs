//! Read-only registry access behind a trait, so importers can be tested with fixtures.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Hive {
    CurrentUser,
    LocalMachine,
}

pub trait Registry: Send + Sync {
    fn get_string(&self, hive: Hive, key: &str, name: &str) -> Option<String>;
    fn get_u32(&self, hive: Hive, key: &str, name: &str) -> Option<u32>;
    /// Names of the subkeys of `key`; `None` when the key does not exist.
    fn subkeys(&self, hive: Hive, key: &str) -> Option<Vec<String>>;
}

/// The real Windows registry.
pub struct WinRegistry;

impl WinRegistry {
    fn open(hive: Hive, key: &str) -> Option<winreg::RegKey> {
        let root = match hive {
            Hive::CurrentUser => winreg::enums::HKEY_CURRENT_USER,
            Hive::LocalMachine => winreg::enums::HKEY_LOCAL_MACHINE,
        };
        winreg::RegKey::predef(root)
            .open_subkey_with_flags(key, winreg::enums::KEY_READ)
            .ok()
    }
}

impl Registry for WinRegistry {
    fn get_string(&self, hive: Hive, key: &str, name: &str) -> Option<String> {
        Self::open(hive, key)?.get_value::<String, _>(name).ok()
    }

    fn get_u32(&self, hive: Hive, key: &str, name: &str) -> Option<u32> {
        Self::open(hive, key)?.get_value::<u32, _>(name).ok()
    }

    fn subkeys(&self, hive: Hive, key: &str) -> Option<Vec<String>> {
        Some(
            Self::open(hive, key)?
                .enum_keys()
                .filter_map(Result::ok)
                .collect(),
        )
    }
}

pub const STEAM_KEY: &str = r"Software\Valve\Steam";

/// `HKCU\Software\Valve\Steam\RunningAppID`, `None` when unset or zero.
pub fn steam_running_app_id(registry: &dyn Registry) -> Option<u32> {
    registry
        .get_u32(Hive::CurrentUser, STEAM_KEY, "RunningAppID")
        .filter(|id| *id != 0)
}

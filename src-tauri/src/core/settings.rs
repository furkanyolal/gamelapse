//! Settings defaults and validation. Persistence lives in the command layer (tauri-plugin-store).

use crate::core::types::{Settings, SettingsUpdate, Theme};
use crate::error::{AppError, AppResult};

pub const POLL_MIN: u32 = 1;
pub const POLL_MAX: u32 = 60;

impl Default for Settings {
    fn default() -> Self {
        Self {
            poll_interval_secs: 5,
            afk_threshold_secs: 600,
            session_merge_gap_secs: 120,
            require_foreground: false,
            tracking_enabled: true,
            theme: Theme::Dark,
        }
    }
}

impl Settings {
    /// Checks every field is in range.
    pub fn validate(&self) -> AppResult<()> {
        if !(POLL_MIN..=POLL_MAX).contains(&self.poll_interval_secs) {
            return Err(AppError::Other(format!(
                "pollIntervalSecs must be between {POLL_MIN} and {POLL_MAX}"
            )));
        }
        if self.afk_threshold_secs < 0 {
            return Err(AppError::Other(
                "afkThresholdSecs must not be negative".into(),
            ));
        }
        if self.session_merge_gap_secs < 0 {
            return Err(AppError::Other(
                "sessionMergeGapSecs must not be negative".into(),
            ));
        }
        Ok(())
    }

    /// Returns a copy with the patch applied (`None` keeps the current value), validated.
    pub fn with_update(&self, patch: &SettingsUpdate) -> AppResult<Settings> {
        let mut next = self.clone();
        if let Some(v) = patch.poll_interval_secs {
            next.poll_interval_secs = v;
        }
        if let Some(v) = patch.afk_threshold_secs {
            next.afk_threshold_secs = v;
        }
        if let Some(v) = patch.session_merge_gap_secs {
            next.session_merge_gap_secs = v;
        }
        if let Some(v) = patch.require_foreground {
            next.require_foreground = v;
        }
        if let Some(v) = patch.tracking_enabled {
            next.tracking_enabled = v;
        }
        if let Some(v) = patch.theme {
            next.theme = v;
        }
        next.validate()?;
        Ok(next)
    }
}

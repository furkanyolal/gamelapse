//! Settings defaults and validation. Persistence lives in the command layer (tauri-plugin-store).

use crate::core::types::{Settings, SettingsUpdate, Theme};
use crate::error::{AppError, AppResult};

pub const POLL_MIN: u32 = 1;
pub const POLL_MAX: u32 = 60;
pub const REMINDER_MIN: u32 = 300;
pub const REMINDER_MAX: u32 = 14_400;

impl Default for Settings {
    fn default() -> Self {
        Self {
            poll_interval_secs: 5,
            afk_threshold_secs: 600,
            session_merge_gap_secs: 120,
            require_foreground: false,
            tracking_enabled: true,
            theme: Theme::Dark,
            import_on_startup: true,
            notifications_enabled: true,
            reminder_interval_secs: 0,
            startup_hint_dismissed: false,
        }
    }
}

/// Adds fields introduced after a settings object was saved, so older files keep their values
/// instead of being reset to defaults.
pub fn backfill_settings(mut value: serde_json::Value) -> serde_json::Value {
    if let Some(obj) = value.as_object_mut() {
        // Phase 3.
        obj.entry("importOnStartup").or_insert(true.into());
        // Phase 4.
        obj.entry("notificationsEnabled").or_insert(true.into());
        obj.entry("reminderIntervalSecs").or_insert(0.into());
        // Phase 6.
        obj.entry("startupHintDismissed").or_insert(false.into());
    }
    value
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
        if self.reminder_interval_secs != 0
            && !(REMINDER_MIN..=REMINDER_MAX).contains(&self.reminder_interval_secs)
        {
            return Err(AppError::Other(format!(
                "reminderIntervalSecs must be 0 or between {REMINDER_MIN} and {REMINDER_MAX}"
            )));
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
        if let Some(v) = patch.import_on_startup {
            next.import_on_startup = v;
        }
        if let Some(v) = patch.notifications_enabled {
            next.notifications_enabled = v;
        }
        if let Some(v) = patch.reminder_interval_secs {
            next.reminder_interval_secs = v;
        }
        if let Some(v) = patch.startup_hint_dismissed {
            next.startup_hint_dismissed = v;
        }
        next.validate()?;
        Ok(next)
    }
}

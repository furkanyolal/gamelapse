use gamelapse_lib::core::settings::backfill_settings;
use gamelapse_lib::core::types::{Settings, SettingsUpdate};
use serde_json::json;

#[test]
fn startup_hint_defaults_to_false_and_can_be_set() {
    let defaults = Settings::default();
    assert!(!defaults.startup_hint_dismissed);
    let next = defaults
        .with_update(&SettingsUpdate {
            startup_hint_dismissed: Some(true),
            ..Default::default()
        })
        .unwrap();
    assert!(next.startup_hint_dismissed);
    // An empty patch keeps it.
    assert!(
        next.with_update(&SettingsUpdate::default())
            .unwrap()
            .startup_hint_dismissed
    );
}

#[test]
fn settings_saved_before_phase_six_keep_their_values() {
    let old = json!({
        "pollIntervalSecs": 7,
        "afkThresholdSecs": 300,
        "sessionMergeGapSecs": 60,
        "requireForeground": true,
        "trackingEnabled": false,
        "theme": "light",
        "importOnStartup": false,
        "notificationsEnabled": false,
        "reminderIntervalSecs": 900
    });
    let settings: Settings = serde_json::from_value(backfill_settings(old)).unwrap();
    assert!(!settings.startup_hint_dismissed);
    assert_eq!(settings.poll_interval_secs, 7);
    assert!(settings.require_foreground);
    assert!(!settings.import_on_startup);
    assert_eq!(settings.reminder_interval_secs, 900);

    // An existing value is never overwritten.
    let kept: Settings = serde_json::from_value(backfill_settings(json!({
        "pollIntervalSecs": 5, "afkThresholdSecs": 600, "sessionMergeGapSecs": 120,
        "requireForeground": false, "trackingEnabled": true, "theme": "dark",
        "startupHintDismissed": true
    })))
    .unwrap();
    assert!(kept.startup_hint_dismissed);
}

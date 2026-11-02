//! Session export to CSV and JSON.

use std::collections::BTreeSet;
use std::io::{BufWriter, Write};
use std::path::Path;

use chrono::{Local, SecondsFormat, TimeZone};
use rusqlite::Connection;
use serde::Serialize;

use crate::core::db::repo;
use crate::core::types::{
    ExportFormat, ExportOptions, ExportResult, Game, SessionQuery, SessionWithGame,
};
use crate::error::AppResult;

pub const CSV_HEADER: [&str; 8] = [
    "session_id",
    "game",
    "started_at",
    "ended_at",
    "duration_secs",
    "afk_secs",
    "source",
    "note",
];

pub fn extension(format: ExportFormat) -> &'static str {
    match format {
        ExportFormat::Csv => "csv",
        ExportFormat::Json => "json",
    }
}

/// `gamelapse-sessions-<YYYYMMDD>.<ext>`, dated by the local day of `now`.
pub fn default_file_name(format: ExportFormat, now: i64) -> String {
    let day = Local
        .timestamp_opt(now, 0)
        .earliest()
        .unwrap_or_else(Local::now)
        .format("%Y%m%d");
    format!("gamelapse-sessions-{day}.{}", extension(format))
}

/// Local ISO 8601 with offset, for example `2026-11-02T20:15:00+03:00`.
fn local_iso(ts: i64) -> String {
    Local
        .timestamp_opt(ts, 0)
        .earliest()
        .map(|d| d.to_rfc3339_opts(SecondsFormat::Secs, false))
        .unwrap_or_default()
}

/// RFC 4180: quote a field holding a comma, quote, CR or LF, doubling inner quotes.
fn csv_field(field: &str) -> String {
    if field.contains([',', '"', '\r', '\n']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

fn csv_row(fields: &[String]) -> String {
    let mut line = fields
        .iter()
        .map(|f| csv_field(f))
        .collect::<Vec<_>>()
        .join(",");
    line.push_str("\r\n");
    line
}

/// Sessions matching the options, oldest first.
fn load_sessions(
    conn: &Connection,
    options: &ExportOptions,
    now: i64,
) -> AppResult<Vec<SessionWithGame>> {
    let query = SessionQuery {
        game_id: options.game_id,
        from: options.from,
        to: options.to,
        limit: u32::MAX,
        offset: 0,
        // An export is the user's own data; hidden games stay in it.
        include_hidden: true,
    };
    let mut sessions = repo::list_sessions(conn, &query, now)?;
    sessions.reverse();
    Ok(sessions)
}

fn write_csv(out: &mut impl Write, sessions: &[SessionWithGame]) -> std::io::Result<()> {
    out.write_all(csv_row(&CSV_HEADER.map(String::from)).as_bytes())?;
    for s in sessions {
        let row = [
            s.session.id.to_string(),
            s.game_name.clone(),
            local_iso(s.session.started_at),
            s.session.ended_at.map(local_iso).unwrap_or_default(),
            s.session.duration_secs.to_string(),
            s.session.afk_secs.to_string(),
            s.session.source.as_str().to_string(),
            s.session.note.clone().unwrap_or_default(),
        ];
        out.write_all(csv_row(&row).as_bytes())?;
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonExport<'a> {
    /// UTC unix seconds, like every other timestamp in the document.
    exported_at: i64,
    games: &'a [Game],
    sessions: &'a [SessionWithGame],
}

/// Writes the sessions matching `options` to `path` in the chosen format.
/// Active sessions are included and counted up to `now`.
pub fn export_to_path(
    conn: &Connection,
    options: &ExportOptions,
    path: &Path,
    now: i64,
) -> AppResult<ExportResult> {
    let sessions = load_sessions(conn, options, now)?;
    let game_ids: BTreeSet<i32> = sessions.iter().map(|s| s.session.game_id).collect();
    let mut out = BufWriter::new(std::fs::File::create(path)?);
    match options.format {
        ExportFormat::Csv => write_csv(&mut out, &sessions)?,
        ExportFormat::Json => {
            let mut games = game_ids
                .iter()
                .map(|id| repo::get_game(conn, *id))
                .collect::<AppResult<Vec<_>>>()?;
            games.sort_by(|a, b| (&a.sort_name, a.id).cmp(&(&b.sort_name, b.id)));
            let doc = JsonExport {
                exported_at: now,
                games: &games,
                sessions: &sessions,
            };
            serde_json::to_writer_pretty(&mut out, &doc).map_err(std::io::Error::from)?;
        }
    }
    out.flush()?;
    Ok(ExportResult {
        path: path.display().to_string(),
        sessions: sessions.len() as i32,
        games: game_ids.len() as i32,
    })
}

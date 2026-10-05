use anyhow::{anyhow, Result};
use chrono::{DateTime, Local, Utc};
use log::{debug, error, info};
use rusqlite::{params, Connection, OptionalExtension};
use rusqlite_migration::{Migrations, M};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::AppHandle;
use tauri_specta::Event;

static MIGRATIONS: &[M] = &[
    M::up(
        "CREATE TABLE IF NOT EXISTS transcription_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_name TEXT NOT NULL,
            timestamp INTEGER NOT NULL,
            saved BOOLEAN NOT NULL DEFAULT 0,
            title TEXT NOT NULL,
            transcription_text TEXT NOT NULL
        );",
    ),
    M::up("ALTER TABLE transcription_history ADD COLUMN post_processed_text TEXT;"),
    M::up("ALTER TABLE transcription_history ADD COLUMN post_process_prompt TEXT;"),
    M::up("ALTER TABLE transcription_history ADD COLUMN post_process_requested BOOLEAN NOT NULL DEFAULT 0;"),

    M::up("ALTER TABLE transcription_history ADD COLUMN model TEXT;"),

    M::up("ALTER TABLE transcription_history ADD COLUMN previous_text TEXT;"),
    M::up("ALTER TABLE transcription_history ADD COLUMN previous_model TEXT;"),

    M::up("ALTER TABLE transcription_history ADD COLUMN revisions TEXT;"),
    M::up("ALTER TABLE transcription_history DROP COLUMN previous_text;"),
    M::up("ALTER TABLE transcription_history DROP COLUMN previous_model;"),

    M::up("ALTER TABLE transcription_history ADD COLUMN transcribed_at INTEGER;"),
];

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct PaginatedHistory {
    pub entries: Vec<HistoryEntry>,
    pub has_more: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type, tauri_specta::Event)]
#[serde(tag = "action")]
pub enum HistoryUpdatePayload {
    #[serde(rename = "added")]
    Added { entry: HistoryEntry },
    #[serde(rename = "updated")]
    Updated { entry: HistoryEntry },
    #[serde(rename = "deleted")]
    Deleted { id: i64 },
    #[serde(rename = "toggled")]
    Toggled { id: i64 },
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct HistoryEntry {
    pub id: i64,
    pub file_name: String,
    pub timestamp: i64,
    pub saved: bool,
    pub title: String,
    pub transcription_text: String,
    pub post_processed_text: Option<String>,
    pub post_process_prompt: Option<String>,
    pub post_process_requested: bool,

    pub model: Option<String>,

    pub revisions: Vec<Revision>,

    pub transcribed_at: Option<i64>,

    pub has_audio: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct Revision {
    pub text: String,

    pub model: Option<String>,

    #[serde(default)]
    pub transcribed_at: Option<i64>,
}

const MAX_REVISIONS: usize = 10;

fn push_revision(raw: Option<&str>, rev: Revision) -> Vec<Revision> {
    let mut list: Vec<Revision> = raw
        .and_then(|r| serde_json::from_str(r).ok())
        .unwrap_or_default();
    list.push(rev);
    if list.len() > MAX_REVISIONS {
        let drop = list.len() - MAX_REVISIONS;
        list.drain(..drop);
    }
    list
}

fn is_recording_file_name(name: &str) -> bool {
    let Some(stamp) = name
        .strip_prefix("anonen-")
        .and_then(|rest| rest.strip_suffix(".wav"))
    else {
        return false;
    };
    !stamp.is_empty() && stamp.bytes().all(|b| b.is_ascii_digit())
}

fn orphan_recordings<'a>(
    on_disk: &'a [String],
    known: &std::collections::HashSet<String>,
) -> Vec<&'a str> {
    on_disk
        .iter()
        .map(String::as_str)
        .filter(|name| is_recording_file_name(name) && !known.contains(*name))
        .collect()
}

pub struct HistoryManager {
    app_handle: AppHandle,
    recordings_dir: PathBuf,
    db_path: PathBuf,
}

impl HistoryManager {
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        let app_data_dir = crate::portable::app_data_dir(app_handle)?;
        let recordings_dir = app_data_dir.join("recordings");
        let db_path = app_data_dir.join("history.db");

        if !recordings_dir.exists() {
            fs::create_dir_all(&recordings_dir)?;
            debug!(
                "Created recordings directory: {}",
                crate::utils::loggable_path(&recordings_dir)
            );
        }

        let manager = Self {
            app_handle: app_handle.clone(),
            recordings_dir,
            db_path,
        };

        manager.init_database()?;

        match manager.sweep_orphan_recordings() {
            Ok(0) => {}
            Ok(n) => info!("Removed {} recording(s) with no history entry", n),
            Err(e) => error!("Could not sweep orphaned recordings: {}", e),
        }

        Ok(manager)
    }

    fn sweep_orphan_recordings(&self) -> Result<usize> {
        let conn = self.get_connection()?;
        let known: std::collections::HashSet<String> = {
            let mut stmt = conn.prepare("SELECT file_name FROM transcription_history")?;
            let rows = stmt
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            rows.into_iter().collect()
        };

        let mut on_disk = Vec::new();
        for entry in fs::read_dir(&self.recordings_dir)? {
            let entry = entry?;

            if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                continue;
            }
            if let Some(name) = entry.file_name().to_str() {
                on_disk.push(name.to_string());
            }
        }

        let mut removed = 0;
        for name in orphan_recordings(&on_disk, &known) {
            match fs::remove_file(self.recordings_dir.join(name)) {
                Ok(()) => {
                    debug!("Removed orphaned recording {}", name);
                    removed += 1;
                }

                Err(e) => error!("Could not remove orphaned recording {}: {}", name, e),
            }
        }
        Ok(removed)
    }

    fn init_database(&self) -> Result<()> {
        info!(
            "Initializing database at {}",
            crate::utils::loggable_path(&self.db_path)
        );

        let mut conn = Self::open_connection(&self.db_path)?;

        self.migrate_from_tauri_plugin_sql(&conn)?;

        let migrations = Migrations::new(MIGRATIONS.to_vec());

        #[cfg(debug_assertions)]
        migrations.validate().expect("Invalid migrations");

        let version_before: i32 =
            conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        debug!("Database version before migration: {}", version_before);

        migrations.to_latest(&mut conn)?;

        let version_after: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;

        if version_after > version_before {
            info!(
                "Database migrated from version {} to {}",
                version_before, version_after
            );
        } else {
            debug!("Database already at latest version {}", version_after);
        }

        Ok(())
    }

    fn migrate_from_tauri_plugin_sql(&self, conn: &Connection) -> Result<()> {
        let has_sqlx_migrations: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='_sqlx_migrations'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !has_sqlx_migrations {
            return Ok(());
        }

        let current_version: i32 =
            conn.pragma_query_value(None, "user_version", |row| row.get(0))?;

        if current_version > 0 {
            return Ok(());
        }

        let old_version: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations WHERE success = 1",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if old_version > 0 {
            info!(
                "Migrating from tauri-plugin-sql (version {}) to rusqlite_migration",
                old_version
            );

            conn.pragma_update(None, "user_version", old_version)?;

            info!(
                "Migration tracking converted: user_version set to {}",
                old_version
            );
        }

        Ok(())
    }

    fn get_connection(&self) -> Result<Connection> {
        Self::open_connection(&self.db_path)
    }

    fn open_connection(db_path: &Path) -> Result<Connection> {
        let conn = Connection::open(db_path)?;
        conn.pragma_update(None, "secure_delete", true)?;
        Ok(conn)
    }

    fn map_history_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<HistoryEntry> {
        Ok(HistoryEntry {
            id: row.get("id")?,
            file_name: row.get("file_name")?,
            timestamp: row.get("timestamp")?,
            saved: row.get("saved")?,
            title: row.get("title")?,
            transcription_text: row.get("transcription_text")?,
            post_processed_text: row.get("post_processed_text")?,
            post_process_prompt: row.get("post_process_prompt")?,
            post_process_requested: row.get("post_process_requested")?,
            model: row.get("model")?,

            revisions: row
                .get::<_, Option<String>>("revisions")?
                .and_then(|raw| serde_json::from_str(&raw).ok())
                .unwrap_or_default(),
            transcribed_at: row.get("transcribed_at")?,

            has_audio: false,
        })
    }

    fn has_recording(&self, file_name: &str) -> bool {
        self.recordings_dir.join(file_name).is_file()
    }

    fn with_audio_flag(&self, mut entry: HistoryEntry) -> HistoryEntry {
        entry.has_audio = self.has_recording(&entry.file_name);
        entry
    }

    pub fn recordings_dir(&self) -> &std::path::Path {
        &self.recordings_dir
    }

    pub fn save_entry(
        &self,
        file_name: String,
        transcription_text: String,
        post_process_requested: bool,
        post_processed_text: Option<String>,
        post_process_prompt: Option<String>,
        model: Option<String>,
    ) -> Result<HistoryEntry> {
        let timestamp = Utc::now().timestamp();
        let title = self.format_timestamp_title(timestamp);

        let conn = self.get_connection()?;
        conn.execute(
            "INSERT INTO transcription_history (
                file_name,
                timestamp,
                saved,
                title,
                transcription_text,
                post_processed_text,
                post_process_prompt,
                post_process_requested,
                model
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                &file_name,
                timestamp,
                false,
                &title,
                &transcription_text,
                &post_processed_text,
                &post_process_prompt,
                post_process_requested,
                &model,
            ],
        )?;

        let has_audio = self.has_recording(&file_name);
        let entry = HistoryEntry {
            id: conn.last_insert_rowid(),
            file_name,
            timestamp,
            saved: false,
            title,
            transcription_text,
            post_processed_text,
            post_process_prompt,
            post_process_requested,
            model,

            revisions: Vec::new(),

            transcribed_at: None,
            has_audio,
        };

        debug!("Saved history entry with id {}", entry.id);

        if let Err(e) = self.cleanup_old_entries() {
            error!("Failed to trim old history entries: {}", e);
        }

        if let Err(e) = (HistoryUpdatePayload::Added {
            entry: entry.clone(),
        })
        .emit(&self.app_handle)
        {
            error!("Failed to emit history-updated event: {}", e);
        }

        Ok(entry)
    }

    pub fn update_transcription(
        &self,
        id: i64,
        transcription_text: String,
        post_processed_text: Option<String>,
        post_process_prompt: Option<String>,

        model: Option<String>,
    ) -> Result<HistoryEntry> {
        let conn = self.get_connection()?;

        let entry = self.with_audio_flag(Self::update_transcription_with_conn(
            &conn,
            id,
            transcription_text,
            post_processed_text,
            post_process_prompt,
            model,
        )?);

        debug!("Updated transcription for history entry {}", id);

        if let Err(e) = (HistoryUpdatePayload::Updated {
            entry: entry.clone(),
        })
        .emit(&self.app_handle)
        {
            error!("Failed to emit history-updated event: {}", e);
        }

        Ok(entry)
    }

    fn update_transcription_with_conn(
        conn: &Connection,
        id: i64,
        transcription_text: String,
        post_processed_text: Option<String>,
        post_process_prompt: Option<String>,
        model: Option<String>,
    ) -> Result<HistoryEntry> {
        let current = conn.query_row(
            "SELECT transcription_text, model, revisions, timestamp, transcribed_at
               FROM transcription_history WHERE id = ?1",
            params![id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                ))
            },
        );
        let (prev_text, prev_model, raw, timestamp, prev_at) = match current {
            Ok(v) => v,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Err(anyhow!("History entry {} not found", id))
            }
            Err(e) => return Err(e.into()),
        };

        let revisions = push_revision(
            raw.as_deref(),
            Revision {
                text: prev_text,
                model: prev_model,
                transcribed_at: Some(prev_at.unwrap_or(timestamp)),
            },
        );

        let updated = conn.execute(
            "UPDATE transcription_history
             SET revisions = ?1,
                 transcription_text = ?2,
                 post_processed_text = ?3,
                 post_process_prompt = ?4,
                 model = coalesce(?5, model),
                 transcribed_at = ?6
             WHERE id = ?7",
            params![
                serde_json::to_string(&revisions)?,
                transcription_text,
                post_processed_text,
                post_process_prompt,
                model,
                Utc::now().timestamp(),
                id
            ],
        )?;

        if updated == 0 {
            return Err(anyhow!("History entry {} not found", id));
        }

        let entry = conn
            .query_row(
                "SELECT id, file_name, timestamp, saved, title, transcription_text, post_processed_text, post_process_prompt, post_process_requested, model, revisions, transcribed_at
                 FROM transcription_history WHERE id = ?1",
                params![id],
                Self::map_history_entry,
            )?;

        Ok(entry)
    }

    pub fn cleanup_old_entries(&self) -> Result<()> {
        match crate::settings::get_history_retention(&self.app_handle) {
            crate::settings::HistoryRetention::None => Ok(()),
            crate::settings::HistoryRetention::Recent => {
                self.cleanup_by_count(crate::settings::HistoryRetention::RECENT_LIMIT)
            }

            crate::settings::HistoryRetention::Unlimited => {
                self.drop_audio_beyond(crate::settings::HistoryRetention::UNLIMITED_AUDIO_LIMIT)
            }
        }
    }

    fn drop_audio_beyond(&self, keep: usize) -> Result<()> {
        let conn = self.get_connection()?;
        let mut dropped = 0;
        for (_, file_name) in rows_losing_audio(&conn, keep)? {
            let path = self.recordings_dir.join(&file_name);
            if !path.is_file() {
                continue;
            }
            match fs::remove_file(&path) {
                Ok(()) => dropped += 1,
                Err(e) => error!("Failed to delete WAV file {}: {}", file_name, e),
            }
        }
        if dropped > 0 {
            debug!("Deleted {} old recording(s); their text stays", dropped);
        }
        Ok(())
    }

    pub fn audio_usage(&self) -> Result<HistoryAudioUsage> {
        Ok(audio_usage_in(&self.recordings_dir)?)
    }

    fn delete_entries_and_files(&self, entries: &[(i64, String)]) -> Result<usize> {
        if entries.is_empty() {
            return Ok(0);
        }

        let conn = self.get_connection()?;
        let mut deleted_count = 0;

        for (id, file_name) in entries {
            conn.execute(
                "DELETE FROM transcription_history WHERE id = ?1",
                params![id],
            )?;

            let file_path = self.recordings_dir.join(file_name);
            if file_path.exists() {
                if let Err(e) = fs::remove_file(&file_path) {
                    error!("Failed to delete WAV file {}: {}", file_name, e);
                } else {
                    debug!("Deleted old WAV file: {}", file_name);
                    deleted_count += 1;
                }
            }
        }

        Ok(deleted_count)
    }

    fn cleanup_by_count(&self, limit: usize) -> Result<()> {
        let conn = self.get_connection()?;
        let entries_to_delete = unsaved_rows_beyond(&conn, limit)?;
        if !entries_to_delete.is_empty() {
            let deleted_count = self.delete_entries_and_files(&entries_to_delete)?;

            if deleted_count > 0 {
                debug!("Cleaned up {} old history entries by count", deleted_count);
            }
        }

        Ok(())
    }

    pub async fn get_history_entries(
        &self,
        cursor: Option<i64>,
        limit: Option<usize>,
        saved_only: bool,
    ) -> Result<PaginatedHistory> {
        let conn = self.get_connection()?;
        let (entries, has_more) = Self::history_page_with_conn(&conn, cursor, limit, saved_only)?;
        let entries = entries
            .into_iter()
            .map(|e| self.with_audio_flag(e))
            .collect();

        Ok(PaginatedHistory { entries, has_more })
    }

    fn history_page_with_conn(
        conn: &Connection,
        cursor: Option<i64>,
        limit: Option<usize>,
        saved_only: bool,
    ) -> Result<(Vec<HistoryEntry>, bool)> {
        let limit = limit.map(|l| l.min(100));

        let mut entries: Vec<HistoryEntry> = match (cursor, limit) {
            (Some(cursor_id), Some(lim)) => {
                let fetch_count = (lim + 1) as i64;
                let mut stmt = conn.prepare(
                    "SELECT id, file_name, timestamp, saved, title, transcription_text, post_processed_text, post_process_prompt, post_process_requested, model, revisions, transcribed_at
                     FROM transcription_history
                     WHERE id < ?1 AND (?3 = 0 OR saved = 1)
                     ORDER BY id DESC
                     LIMIT ?2",
                )?;
                let result = stmt
                    .query_map(
                        params![cursor_id, fetch_count, saved_only],
                        Self::map_history_entry,
                    )?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                result
            }
            (None, Some(lim)) => {
                let fetch_count = (lim + 1) as i64;
                let mut stmt = conn.prepare(
                    "SELECT id, file_name, timestamp, saved, title, transcription_text, post_processed_text, post_process_prompt, post_process_requested, model, revisions, transcribed_at
                     FROM transcription_history
                     WHERE (?2 = 0 OR saved = 1)
                     ORDER BY id DESC
                     LIMIT ?1",
                )?;
                let result = stmt
                    .query_map(params![fetch_count, saved_only], Self::map_history_entry)?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                result
            }
            (_, None) => {
                let mut stmt = conn.prepare(
                    "SELECT id, file_name, timestamp, saved, title, transcription_text, post_processed_text, post_process_prompt, post_process_requested, model, revisions, transcribed_at
                     FROM transcription_history
                     WHERE (?1 = 0 OR saved = 1)
                     ORDER BY id DESC",
                )?;
                let result = stmt
                    .query_map(params![saved_only], Self::map_history_entry)?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                result
            }
        };

        let has_more = limit.is_some_and(|lim| entries.len() > lim);
        if has_more {
            entries.pop();
        }
        Ok((entries, has_more))
    }

    #[cfg(test)]
    fn get_latest_entry_with_conn(conn: &Connection) -> Result<Option<HistoryEntry>> {
        let mut stmt = conn.prepare(
            "SELECT
                id,
                file_name,
                timestamp,
                saved,
                title,
                transcription_text,
                post_processed_text,
                post_process_prompt,
                post_process_requested,
                model,
                revisions,
                transcribed_at
             FROM transcription_history
             ORDER BY id DESC
             LIMIT 1",
        )?;

        let entry = stmt.query_row([], Self::map_history_entry).optional()?;
        Ok(entry)
    }

    pub fn get_latest_completed_entry(&self) -> Result<Option<HistoryEntry>> {
        let conn = self.get_connection()?;
        Ok(Self::get_latest_completed_entry_with_conn(&conn)?.map(|e| self.with_audio_flag(e)))
    }

    fn get_latest_completed_entry_with_conn(conn: &Connection) -> Result<Option<HistoryEntry>> {
        let mut stmt = conn.prepare(
            "SELECT
                id,
                file_name,
                timestamp,
                saved,
                title,
                transcription_text,
                post_processed_text,
                post_process_prompt,
                post_process_requested,
                model,
                revisions,
                transcribed_at
             FROM transcription_history
             WHERE transcription_text != ''
             ORDER BY id DESC
             LIMIT 1",
        )?;

        let entry = stmt.query_row([], Self::map_history_entry).optional()?;
        Ok(entry)
    }

    pub async fn toggle_saved_status(&self, id: i64) -> Result<()> {
        let conn = self.get_connection()?;

        let current_saved: bool = conn.query_row(
            "SELECT saved FROM transcription_history WHERE id = ?1",
            params![id],
            |row| row.get("saved"),
        )?;

        let new_saved = !current_saved;

        conn.execute(
            "UPDATE transcription_history SET saved = ?1 WHERE id = ?2",
            params![new_saved, id],
        )?;

        debug!("Toggled saved status for entry {}: {}", id, new_saved);

        if let Err(e) = (HistoryUpdatePayload::Toggled { id }).emit(&self.app_handle) {
            error!("Failed to emit history-updated event: {}", e);
        }

        Ok(())
    }

    pub fn get_audio_file_path(&self, file_name: &str) -> PathBuf {
        match Path::new(file_name).file_name() {
            Some(name) => self.recordings_dir.join(name),

            None => self.recordings_dir.clone(),
        }
    }

    pub async fn get_entry_by_id(&self, id: i64) -> Result<Option<HistoryEntry>> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT
                id,
                file_name,
                timestamp,
                saved,
                title,
                transcription_text,
                post_processed_text,
                post_process_prompt,
                post_process_requested,
                model,
                revisions,
                transcribed_at
             FROM transcription_history
             WHERE id = ?1",
        )?;

        let entry = stmt.query_row([id], Self::map_history_entry).optional()?;

        Ok(entry.map(|e| self.with_audio_flag(e)))
    }

    pub async fn delete_entry(&self, id: i64) -> Result<()> {
        let conn = self.get_connection()?;

        if let Some(entry) = self.get_entry_by_id(id).await? {
            let file_path = self.get_audio_file_path(&entry.file_name);
            if file_path.exists() {
                if let Err(e) = fs::remove_file(&file_path) {
                    error!("Failed to delete audio file {}: {}", entry.file_name, e);
                }
            }
        }

        conn.execute(
            "DELETE FROM transcription_history WHERE id = ?1",
            params![id],
        )?;

        debug!("Deleted history entry with id: {}", id);

        if let Err(e) = (HistoryUpdatePayload::Deleted { id }).emit(&self.app_handle) {
            error!("Failed to emit history-updated event: {}", e);
        }

        Ok(())
    }

    pub fn clear_all_entries(&self) -> Result<usize> {
        let conn = self.get_connection()?;

        let entries: Vec<(i64, String)> = {
            let mut stmt = conn.prepare("SELECT id, file_name FROM transcription_history")?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, i64>("id")?, row.get::<_, String>("file_name")?))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            rows
        };

        let count = entries.len();

        conn.execute("DELETE FROM transcription_history", [])?;

        for (_, file_name) in &entries {
            let file_path = self.recordings_dir.join(file_name);
            if file_path.exists() {
                if let Err(e) = fs::remove_file(&file_path) {
                    error!("Failed to delete WAV file {}: {}", file_name, e);
                }
            }
        }

        if let Err(e) = conn.execute_batch("VACUUM") {
            error!("Failed to vacuum history database after clear-all: {}", e);
        }

        debug!("Cleared all {} history entries", count);

        Ok(count)
    }

    fn format_timestamp_title(&self, timestamp: i64) -> String {
        if let Some(utc_datetime) = DateTime::from_timestamp(timestamp, 0) {
            let local_datetime = utc_datetime.with_timezone(&Local);
            local_datetime.format("%B %e, %Y - %l:%M%p").to_string()
        } else {
            format!("Recording {}", timestamp)
        }
    }
}

fn rows_losing_audio(conn: &Connection, keep: usize) -> Result<Vec<(i64, String)>> {
    let mut stmt = conn.prepare(
        "SELECT id, file_name, transcription_text FROM transcription_history WHERE saved = 0 ORDER BY id DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>("id")?,
            row.get::<_, String>("file_name")?,
            row.get::<_, String>("transcription_text")?,
        ))
    })?;
    let mut losing = Vec::new();
    for (index, row) in rows.enumerate() {
        let (id, file_name, text) = row?;
        if index >= keep && !text.trim().is_empty() {
            losing.push((id, file_name));
        }
    }
    Ok(losing)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
pub struct HistoryAudioUsage {
    pub count: u32,
    pub bytes: u64,
}

fn audio_usage_in(dir: &std::path::Path) -> std::io::Result<HistoryAudioUsage> {
    let mut usage = HistoryAudioUsage { count: 0, bytes: 0 };
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let is_ours = entry
            .file_name()
            .to_str()
            .is_some_and(is_recording_file_name);
        if !is_ours {
            continue;
        }
        usage.count += 1;
        usage.bytes += entry.metadata().map(|m| m.len()).unwrap_or(0);
    }
    Ok(usage)
}

fn unsaved_rows_beyond(conn: &Connection, limit: usize) -> Result<Vec<(i64, String)>> {
    let mut stmt = conn.prepare(
        "SELECT id, file_name FROM transcription_history WHERE saved = 0 ORDER BY id DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, i64>("id")?, row.get::<_, String>("file_name")?))
    })?;
    let mut entries: Vec<(i64, String)> = Vec::new();
    for row in rows {
        entries.push(row?);
    }
    Ok(entries.into_iter().skip(limit).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{params, Connection};
    use std::collections::HashSet;

    fn known(names: &[&str]) -> HashSet<String> {
        names.iter().map(|n| n.to_string()).collect()
    }

    fn on_disk(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| n.to_string()).collect()
    }

    #[test]
    fn a_recording_that_still_has_a_history_row_is_never_swept() {
        let files = on_disk(&["anonen-1.wav", "anonen-2.wav"]);
        let orphans = orphan_recordings(&files, &known(&["anonen-1.wav", "anonen-2.wav"]));
        assert!(orphans.is_empty(), "{orphans:?}");
    }

    #[test]
    fn a_recording_with_no_history_row_is_swept() {
        let files = on_disk(&["anonen-1.wav", "anonen-2.wav"]);
        assert_eq!(
            orphan_recordings(&files, &known(&["anonen-1.wav"])),
            vec!["anonen-2.wav"]
        );
    }

    #[test]
    fn files_that_are_not_ours_are_left_alone() {
        let files = on_disk(&[
            "notes.txt",
            "important.wav",
            "anonen-.wav",
            "anonen-abc.wav",
            "anonen-1.wav.bak",
            "handy-1.wav",
            "ANONEN-1.WAV",
        ]);
        assert!(
            orphan_recordings(&files, &known(&[])).is_empty(),
            "{:?}",
            orphan_recordings(&files, &known(&[]))
        );
    }

    #[test]
    fn a_failed_trim_does_not_turn_a_saved_entry_into_a_failed_save() {
        let source = include_str!("history.rs").replace("\r\n", "\n");
        let save = source
            .split("pub fn save_entry(")
            .nth(1)
            .and_then(|body| body.split("\n    }\n").next())
            .expect("save_entry is in this file");
        assert!(save.contains("INSERT INTO transcription_history"));

        assert_eq!(save.matches("cleanup_old_entries()").count(), 1);
        let trim = "        if let Err(e) = self.cleanup_old_entries() {\n            \
                    error!(\"Failed to trim old history entries: {}\", e);\n        }\n";
        assert!(
            save.contains(trim),
            "save_entry must only log the trim's error: the row is already in"
        );
    }

    fn insert_at(conn: &Connection, timestamp: i64, saved: bool) -> i64 {
        conn.execute(
            "INSERT INTO transcription_history (file_name, timestamp, saved, title, transcription_text)
             VALUES (?1, ?2, ?3, 't', 'x')",
            params![format!("anonen-{timestamp}.wav"), timestamp, saved],
        )
        .expect("insert");
        conn.last_insert_rowid()
    }

    #[test]
    fn the_trim_never_removes_the_row_that_was_just_saved() {
        let conn = setup_conn();
        let future: Vec<i64> = (0..20)
            .map(|n| insert_at(&conn, 2_000_000_000 + n, false))
            .collect();
        let just_saved = insert_at(&conn, 1_700_000_000, false);

        let removed = unsaved_rows_beyond(&conn, 20).expect("query");

        let removed_ids: Vec<i64> = removed.iter().map(|(id, _)| *id).collect();
        assert!(!removed_ids.contains(&just_saved), "{removed_ids:?}");

        assert_eq!(removed_ids, vec![future[0]]);
    }

    #[test]
    fn the_trim_keeps_the_newest_rows_and_leaves_starred_ones_alone() {
        let conn = setup_conn();
        let starred = insert_at(&conn, 1, true);
        let ids: Vec<i64> = (0..5).map(|n| insert_at(&conn, 10 + n, false)).collect();

        let removed: Vec<i64> = unsaved_rows_beyond(&conn, 3)
            .expect("query")
            .into_iter()
            .map(|(id, _)| id)
            .collect();

        assert_eq!(removed, vec![ids[1], ids[0]]);
        assert!(!removed.contains(&starred));
        assert!(unsaved_rows_beyond(&conn, 5).expect("query").is_empty());
        assert_eq!(unsaved_rows_beyond(&conn, 0).expect("query").len(), 5);
    }

    fn insert_text(conn: &Connection, timestamp: i64, saved: bool, text: &str) -> i64 {
        conn.execute(
            "INSERT INTO transcription_history (file_name, timestamp, saved, title, transcription_text)
             VALUES (?1, ?2, ?3, 't', ?4)",
            params![format!("anonen-{timestamp}.wav"), timestamp, saved, text],
        )
        .expect("insert");
        conn.last_insert_rowid()
    }

    #[test]
    fn the_trim_of_recordings_keeps_the_newest_and_leaves_starred_and_untranscribed_ones() {
        let conn = setup_conn();
        let starred = insert_text(&conn, 1, true, "starred");
        let untranscribed = insert_text(&conn, 2, false, "  ");
        let ids: Vec<i64> = (0..5)
            .map(|n| insert_text(&conn, 10 + n, false, "x"))
            .collect();

        let losing: Vec<i64> = rows_losing_audio(&conn, 3)
            .expect("query")
            .into_iter()
            .map(|(id, _)| id)
            .collect();

        assert_eq!(losing, vec![ids[1], ids[0]]);
        assert!(!losing.contains(&starred));
        assert!(!losing.contains(&untranscribed));
    }

    #[test]
    fn the_trim_of_recordings_does_nothing_within_the_limit() {
        let conn = setup_conn();
        for n in 0..3 {
            insert_text(&conn, n, false, "x");
        }
        assert!(rows_losing_audio(&conn, 3).expect("query").is_empty());
        assert_eq!(rows_losing_audio(&conn, 0).expect("query").len(), 3);
    }

    #[test]
    fn the_trim_of_recordings_leaves_the_rows_and_their_text() {
        let source = include_str!("history.rs");
        let start = source
            .find("fn drop_audio_beyond")
            .expect("drop_audio_beyond");
        let end = start
            + source[start..]
                .find("pub fn audio_usage")
                .expect("audio_usage");
        let body = &source[start..end];
        assert!(body.contains("fs::remove_file"));
        assert!(!body.contains("DELETE"), "{body}");
        assert!(!body.contains("UPDATE"), "{body}");
    }

    #[test]
    fn recordings_usage_counts_only_our_wav_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::write(dir.path().join("anonen-1756600000.wav"), [0u8; 10]).expect("write");
        fs::write(dir.path().join("anonen-1756600001.wav"), [0u8; 20]).expect("write");
        fs::write(dir.path().join("notes.txt"), [0u8; 99]).expect("write");
        fs::create_dir(dir.path().join("anonen-1756600002.wav")).expect("mkdir");

        let usage = audio_usage_in(dir.path()).expect("usage");

        assert_eq!(
            usage,
            HistoryAudioUsage {
                count: 2,
                bytes: 30
            }
        );
    }

    #[test]
    fn retranscribed_entry_is_sent_with_its_audio_flag() {
        let source = include_str!("history.rs");
        let start = source
            .find("pub fn update_transcription(")
            .expect("update_transcription");
        let end = start
            + source[start..]
                .find("fn update_transcription_with_conn(")
                .expect("update_transcription_with_conn");
        let body = &source[start..end];
        let flag = body
            .find("with_audio_flag(")
            .expect("取り直した行に has_audio を入れていない");
        let emit = body.find(".emit(").expect("画面へ送っていない");
        assert!(flag < emit, "画面へ送る前に has_audio を入れていない");
    }

    #[test]
    fn update_transcription_with_conn_rewrites_the_row_and_keeps_the_old_text() {
        let conn = setup_conn();
        conn.execute(
            "INSERT INTO transcription_history (file_name, timestamp, title, transcription_text, model)
             VALUES ('anonen-1.wav', 1700000000, 't', 'before', 'old-model')",
            [],
        )
        .expect("insert");

        let entry = HistoryManager::update_transcription_with_conn(
            &conn,
            1,
            "after".to_string(),
            None,
            None,
            Some("new-model".to_string()),
        )
        .expect("update");

        assert_eq!(entry.transcription_text, "after");
        assert_eq!(entry.model.as_deref(), Some("new-model"));
        assert_eq!(entry.file_name, "anonen-1.wav");
        assert_eq!(entry.revisions.len(), 1);
        assert_eq!(entry.revisions[0].text, "before");

        assert!(!entry.has_audio);
    }

    #[test]
    fn our_own_name_shape_is_recognised() {
        assert!(is_recording_file_name("anonen-1756600000.wav"));
        assert!(!is_recording_file_name("anonen-1756600000.wav.tmp"));
        assert!(!is_recording_file_name("anonen-1756-600000.wav"));
    }

    fn setup_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory db");
        conn.execute_batch(
            "CREATE TABLE transcription_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_name TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                saved BOOLEAN NOT NULL DEFAULT 0,
                title TEXT NOT NULL,
                transcription_text TEXT NOT NULL,
                post_processed_text TEXT,
                post_process_prompt TEXT,
                post_process_requested BOOLEAN NOT NULL DEFAULT 0,
                model TEXT,
                revisions TEXT,
                transcribed_at INTEGER
            );",
        )
        .expect("create transcription_history table");
        conn
    }

    fn retranscribe(
        conn: &Connection,
        id: i64,
        text: &str,
        model: Option<&str>,

        now: i64,
    ) -> (String, Option<String>, Vec<Revision>) {
        let (prev_text, prev_model, raw, timestamp, prev_at) = conn
            .query_row(
                "SELECT transcription_text, model, revisions, timestamp, transcribed_at
                   FROM transcription_history WHERE id = ?1",
                params![id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, Option<i64>>(4)?,
                    ))
                },
            )
            .expect("read entry");
        let revisions = push_revision(
            raw.as_deref(),
            Revision {
                text: prev_text,
                model: prev_model,
                transcribed_at: Some(prev_at.unwrap_or(timestamp)),
            },
        );
        conn.execute(
            "UPDATE transcription_history
             SET revisions = ?1, transcription_text = ?2, model = coalesce(?3, model),
                 transcribed_at = ?4
             WHERE id = ?5",
            params![
                serde_json::to_string(&revisions).unwrap(),
                text,
                model,
                now,
                id
            ],
        )
        .expect("update entry");
        conn.query_row(
            "SELECT transcription_text, model, revisions FROM transcription_history WHERE id = ?1",
            params![id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            },
        )
        .map(|(t, m, r)| {
            (
                t,
                m,
                r.and_then(|raw| serde_json::from_str(&raw).ok())
                    .unwrap_or_default(),
            )
        })
        .expect("read back")
    }

    fn seed_model(conn: &Connection, model: &str) {
        conn.execute(
            "UPDATE transcription_history SET model = ?1",
            params![model],
        )
        .expect("seed model");
    }

    #[test]
    fn retranscribe_replaces_the_model_name() {
        let conn = setup_conn();
        insert_entry(&conn, 1, "before", None);
        seed_model(&conn, "anonen-cloud:openai/gpt-transcribe");
        let (_, model, _) = retranscribe(
            &conn,
            1,
            "after",
            Some("anonen-cloud:alibaba-funasr/fun-asr-flash-2026-06-15"),
            999,
        );
        assert_eq!(
            model.as_deref(),
            Some("anonen-cloud:alibaba-funasr/fun-asr-flash-2026-06-15")
        );
    }

    #[test]
    fn retranscribe_keeps_the_old_name_when_the_model_is_unknown() {
        let conn = setup_conn();
        insert_entry(&conn, 1, "before", None);
        seed_model(&conn, "anonen-cloud:openai/gpt-transcribe");
        let (_, model, _) = retranscribe(&conn, 1, "after", None, 999);
        assert_eq!(model.as_deref(), Some("anonen-cloud:openai/gpt-transcribe"));
    }

    #[test]
    fn retranscribe_replaces_the_model_name_with_a_local_one() {
        let conn = setup_conn();
        insert_entry(&conn, 1, "before", None);
        seed_model(&conn, "anonen-cloud:openai/gpt-transcribe");
        let (_, model, _) = retranscribe(&conn, 1, "after", Some("large"), 999);
        assert_eq!(model.as_deref(), Some("large"));
    }

    #[test]
    fn retranscribe_keeps_every_previous_result_in_order() {
        let conn = setup_conn();
        insert_entry(&conn, 1, "first", None);
        seed_model(&conn, "voxtral");
        retranscribe(&conn, 1, "second", Some("large"), 999);
        let (text, model, revisions) = retranscribe(&conn, 1, "third", Some("turbo"), 999);

        assert_eq!(text, "third");
        assert_eq!(model.as_deref(), Some("turbo"));

        let seen: Vec<_> = revisions
            .iter()
            .map(|r| (r.text.as_str(), r.model.as_deref()))
            .collect();
        assert_eq!(
            seen,
            vec![("first", Some("voxtral")), ("second", Some("large"))]
        );
    }

    #[test]
    fn paging_times_follow_each_generation() {
        let conn = setup_conn();
        insert_entry(&conn, 100, "first", None);
        retranscribe(&conn, 1, "second", None, 200);
        let (_, _, revisions) = retranscribe(&conn, 1, "third", None, 300);

        let (timestamp, transcribed_at) = conn
            .query_row(
                "SELECT timestamp, transcribed_at FROM transcription_history WHERE id = 1",
                [],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<i64>>(1)?)),
            )
            .expect("read times");
        assert_eq!(timestamp, 100);
        assert_eq!(transcribed_at, Some(300));
        let times: Vec<_> = revisions.iter().map(|r| r.transcribed_at).collect();

        assert_eq!(times, vec![Some(100), Some(200)]);
    }

    #[test]
    fn old_revisions_without_a_time_still_parse() {
        let raw = r#"[{"text":"old","model":null}]"#;
        let list = push_revision(
            Some(raw),
            Revision {
                text: "new".into(),
                model: None,
                transcribed_at: Some(5),
            },
        );
        assert_eq!(list[0].transcribed_at, None);
        assert_eq!(list[1].transcribed_at, Some(5));
    }

    #[test]
    fn revisions_are_capped_and_drop_the_oldest() {
        let mut list: Vec<Revision> = Vec::new();
        let mut raw = None;
        for i in 0..MAX_REVISIONS + 3 {
            list = push_revision(
                raw.as_deref(),
                Revision {
                    text: format!("v{}", i),
                    model: None,
                    transcribed_at: None,
                },
            );
            raw = Some(serde_json::to_string(&list).unwrap());
        }
        assert_eq!(list.len(), MAX_REVISIONS);
        assert_eq!(list.first().unwrap().text, "v3");
        assert_eq!(list.last().unwrap().text, format!("v{}", MAX_REVISIONS + 2));
    }

    #[test]
    fn a_broken_revisions_column_is_treated_as_empty() {
        let list = push_revision(
            Some("{ これは JSON ではない"),
            Revision {
                text: "now".into(),
                model: None,
                transcribed_at: None,
            },
        );
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].text, "now");
    }

    fn insert_entry(conn: &Connection, timestamp: i64, text: &str, post_processed: Option<&str>) {
        conn.execute(
            "INSERT INTO transcription_history (
                file_name,
                timestamp,
                saved,
                title,
                transcription_text,
                post_processed_text,
                post_process_prompt,
                post_process_requested
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                format!("anonen-{}.wav", timestamp),
                timestamp,
                false,
                format!("Recording {}", timestamp),
                text,
                post_processed,
                Option::<String>::None,
                false,
            ],
        )
        .expect("insert history entry");
    }

    #[test]
    fn get_latest_entry_returns_none_when_empty() {
        let conn = setup_conn();
        let entry = HistoryManager::get_latest_entry_with_conn(&conn).expect("fetch latest entry");
        assert!(entry.is_none());
    }

    #[test]
    fn get_latest_entry_returns_newest_entry() {
        let conn = setup_conn();
        insert_entry(&conn, 100, "first", None);
        insert_entry(&conn, 200, "second", Some("processed"));

        let entry = HistoryManager::get_latest_entry_with_conn(&conn)
            .expect("fetch latest entry")
            .expect("entry exists");

        assert_eq!(entry.timestamp, 200);
        assert_eq!(entry.transcription_text, "second");
        assert_eq!(entry.post_processed_text.as_deref(), Some("processed"));
    }

    #[test]
    fn the_latest_entry_is_the_last_one_saved_even_after_the_clock_went_back() {
        let conn = setup_conn();
        insert_entry(&conn, 2_000_000_000, "from the future", None);
        insert_entry(&conn, 1_700_000_000, "just now", None);

        let latest = HistoryManager::get_latest_completed_entry_with_conn(&conn)
            .expect("fetch")
            .expect("exists");
        assert_eq!(latest.transcription_text, "just now");
        let any = HistoryManager::get_latest_entry_with_conn(&conn)
            .expect("fetch")
            .expect("exists");
        assert_eq!(any.transcription_text, "just now");
    }

    #[test]
    fn get_latest_completed_entry_skips_empty_entries() {
        let conn = setup_conn();
        insert_entry(&conn, 100, "completed", None);
        insert_entry(&conn, 200, "", None);

        let entry = HistoryManager::get_latest_completed_entry_with_conn(&conn)
            .expect("fetch latest completed entry")
            .expect("completed entry exists");

        assert_eq!(entry.timestamp, 100);
        assert_eq!(entry.transcription_text, "completed");
    }

    fn ids(entries: &[HistoryEntry]) -> Vec<i64> {
        entries.iter().map(|e| e.id).collect()
    }

    #[test]
    fn saved_only_pages_through_the_starred_rows_only() {
        let conn = setup_conn();
        for i in 1..=7 {
            insert_entry(&conn, 100 + i, &format!("entry {}", i), None);
        }

        for id in [1, 3, 6] {
            conn.execute(
                "UPDATE transcription_history SET saved = 1 WHERE id = ?1",
                params![id],
            )
            .expect("star an entry");
        }

        let (first, more) =
            HistoryManager::history_page_with_conn(&conn, None, Some(2), true).expect("first page");
        assert_eq!(ids(&first), vec![6, 3]);
        assert!(more, "★ の行がまだ残っている");

        let (second, more) = HistoryManager::history_page_with_conn(&conn, Some(3), Some(2), true)
            .expect("second page");
        assert_eq!(ids(&second), vec![1]);
        assert!(!more, "★ の行はもう無い");

        let (all_saved, more) =
            HistoryManager::history_page_with_conn(&conn, None, None, true).expect("all starred");
        assert_eq!(ids(&all_saved), vec![6, 3, 1]);
        assert!(!more);

        let (page, more) = HistoryManager::history_page_with_conn(&conn, None, Some(3), false)
            .expect("unfiltered page");
        assert_eq!(ids(&page), vec![7, 6, 5]);
        assert!(more);
        let (rest, more) = HistoryManager::history_page_with_conn(&conn, Some(5), Some(10), false)
            .expect("unfiltered rest");
        assert_eq!(ids(&rest), vec![4, 3, 2, 1]);
        assert!(!more);
    }

    #[test]
    fn deleted_transcripts_do_not_remain_in_the_database_file() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let db_path = dir.path().join("history.db");
        let secret = "けしたはずのてんしゃ-XYZZY-3141592653";

        let conn = HistoryManager::open_connection(&db_path).expect("open history db");
        conn.execute_batch(
            "CREATE TABLE transcription_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_name TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                saved BOOLEAN NOT NULL DEFAULT 0,
                title TEXT NOT NULL,
                transcription_text TEXT NOT NULL
            );",
        )
        .expect("create transcription_history table");

        let secure_delete: i32 = conn
            .pragma_query_value(None, "secure_delete", |row| row.get(0))
            .expect("read secure_delete pragma");
        assert_eq!(secure_delete, 1, "secure_delete が有効になっていない");

        conn.execute(
            "INSERT INTO transcription_history
                (file_name, timestamp, saved, title, transcription_text)
             VALUES ('anonen-1.wav', 1, 0, 'title', ?1)",
            params![secret],
        )
        .expect("insert entry");

        conn.execute("DELETE FROM transcription_history", [])
            .expect("delete entries");
        conn.execute_batch("VACUUM").expect("vacuum");
        drop(conn);

        let raw = std::fs::read(&db_path).expect("read history.db");
        let leaked = raw.windows(secret.len()).any(|w| w == secret.as_bytes());
        assert!(!leaked, "削除した転写テキストが history.db に残っている");
    }
}

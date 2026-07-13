use chrono::{DateTime, FixedOffset, Local, Utc};
use clap::{Args, Parser, Subcommand};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;
use serde_json::json;
use std::{env, fs, path::PathBuf};
use uuid::Uuid;

type AppResult<T> = Result<T, String>;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS scheduled_sessions (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  type TEXT NOT NULL,
  focus_json TEXT NOT NULL DEFAULT '[]',
  status TEXT NOT NULL CHECK(status IN ('planned','in_progress','completed','cancelled')),
  planned_start_at TEXT NOT NULL,
  planned_duration_min INTEGER,
  actual_start_at TEXT,
  actual_end_at TEXT,
  target_rpe REAL,
  workout_id TEXT,
  notes TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_scheduled_sessions_start ON scheduled_sessions(planned_start_at);
CREATE INDEX IF NOT EXISTS idx_scheduled_sessions_status ON scheduled_sessions(status);
"#;

#[derive(Parser)]
#[command(name = "training-schedule", version, about = "Temporal training-session contract for agents.")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Init,
    Add(AddArgs),
    List(ListArgs),
    Cancel { id: String },
    Start { id: String, #[arg(long)] at: Option<String> },
    Complete {
        id: String,
        #[arg(long)] at: Option<String>,
        #[arg(long)] workout_id: Option<String>,
    },
    Context { #[arg(long)] at: Option<String> },
}

#[derive(Args)]
struct AddArgs {
    #[arg(long)] starts_at: String,
    #[arg(long)] title: String,
    #[arg(long, default_value = "gym")] r#type: String,
    #[arg(long)] duration_min: Option<i64>,
    #[arg(long)] target_rpe: Option<f64>,
    #[arg(long, value_delimiter = ',')] focus: Vec<String>,
    #[arg(long)] notes: Option<String>,
}

#[derive(Args)]
struct ListArgs {
    #[arg(long)] from: Option<String>,
    #[arg(long)] to: Option<String>,
    #[arg(long)] status: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct Session {
    id: String,
    title: String,
    #[serde(rename = "type")]
    session_type: String,
    focus: Vec<String>,
    status: String,
    planned_start_at: String,
    planned_duration_min: Option<i64>,
    actual_start_at: Option<String>,
    actual_end_at: Option<String>,
    target_rpe: Option<f64>,
    workout_id: Option<String>,
    notes: Option<String>,
    created_at: String,
    updated_at: String,
}

fn main() {
    if let Err(message) = run() {
        println!("{}", json!({"ok": false, "error": {"code": "SCHEDULE_ERROR", "message": message}}));
        std::process::exit(1);
    }
}

fn run() -> AppResult<()> {
    let cli = Cli::parse();
    let conn = open_db()?;
    let data = match cli.command {
        Command::Init => json!({"initialized": true, "database": database_path()}),
        Command::Add(args) => json!({"session": add_session(&conn, args)?}),
        Command::List(args) => json!({"sessions": list_sessions(&conn, args)?}),
        Command::Cancel { id } => json!({"session": transition(&conn, &id, "cancelled", None, None, None)?}),
        Command::Start { id, at } => {
            let instant = parse_or_now(at.as_deref())?.to_rfc3339();
            json!({"session": transition(&conn, &id, "in_progress", Some(instant), None, None)?})
        }
        Command::Complete { id, at, workout_id } => {
            let instant = parse_or_now(at.as_deref())?.to_rfc3339();
            json!({"session": transition(&conn, &id, "completed", None, Some(instant), workout_id)?})
        }
        Command::Context { at } => context(&conn, parse_or_now(at.as_deref())?)?,
    };
    println!("{}", json!({"ok": true, "data": data}));
    Ok(())
}

fn database_path() -> String {
    let cwd = env::current_dir().unwrap_or_default();
    let root = if env::var("TRAINING_CLI_LOCAL").ok().as_deref() == Some("1") {
        cwd.join(".training")
    } else if let Ok(home) = env::var("TRAINING_CLI_HOME") {
        PathBuf::from(home)
    } else if cwd.join(".training").exists() {
        cwd.join(".training")
    } else {
        PathBuf::from(env::var("HOME").unwrap_or_else(|_| ".".into())).join(".training-cli")
    };
    root.join("training.db").display().to_string()
}

fn open_db() -> AppResult<Connection> {
    let path = PathBuf::from(database_path());
    if let Some(parent) = path.parent() { fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    conn.pragma_update(None, "foreign_keys", "ON").map_err(|e| e.to_string())?;
    conn.execute_batch(SCHEMA).map_err(|e| e.to_string())?;
    Ok(conn)
}

fn add_session(conn: &Connection, args: AddArgs) -> AppResult<Session> {
    let starts_at = parse_time(&args.starts_at)?.to_rfc3339();
    if args.title.trim().is_empty() { return Err("title is required".into()); }
    if args.duration_min.is_some_and(|v| v <= 0) { return Err("duration-min must be positive".into()); }
    if args.target_rpe.is_some_and(|v| !(1.0..=10.0).contains(&v)) { return Err("target-rpe must be between 1 and 10".into()); }
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let focus = serde_json::to_string(&args.focus).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO scheduled_sessions (id,title,type,focus_json,status,planned_start_at,planned_duration_min,target_rpe,notes,created_at,updated_at) VALUES (?,?,?,?, 'planned',?,?,?,?,?,?)",
        params![id, args.title.trim(), args.r#type, focus, starts_at, args.duration_min, args.target_rpe, args.notes, now, now],
    ).map_err(|e| e.to_string())?;
    get_session(conn, &id)?.ok_or_else(|| "created session could not be read".into())
}

fn list_sessions(conn: &Connection, args: ListArgs) -> AppResult<Vec<Session>> {
    let from = args.from.as_deref().map(parse_time).transpose()?.map(|v| v.to_rfc3339());
    let to = args.to.as_deref().map(parse_time).transpose()?.map(|v| v.to_rfc3339());
    if let Some(status) = args.status.as_deref() { validate_status(status)?; }
    let mut stmt = conn.prepare(
        "SELECT id,title,type,focus_json,status,planned_start_at,planned_duration_min,actual_start_at,actual_end_at,target_rpe,workout_id,notes,created_at,updated_at FROM scheduled_sessions WHERE (?1 IS NULL OR planned_start_at >= ?1) AND (?2 IS NULL OR planned_start_at <= ?2) AND (?3 IS NULL OR status = ?3) ORDER BY planned_start_at"
    ).map_err(|e| e.to_string())?;
    stmt.query_map(params![from, to, args.status], map_session).map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

fn transition(conn: &Connection, id: &str, next: &str, start: Option<String>, end: Option<String>, workout_id: Option<String>) -> AppResult<Session> {
    let current = get_session(conn, id)?.ok_or_else(|| format!("session not found: {id}"))?;
    match (current.status.as_str(), next) {
        ("planned", "in_progress" | "cancelled" | "completed") | ("in_progress", "completed") => {}
        _ => return Err(format!("invalid transition: {} -> {next}", current.status)),
    }
    conn.execute(
        "UPDATE scheduled_sessions SET status=?, actual_start_at=COALESCE(?,actual_start_at), actual_end_at=COALESCE(?,actual_end_at), workout_id=COALESCE(?,workout_id), updated_at=? WHERE id=?",
        params![next, start, end, workout_id, Utc::now().to_rfc3339(), id],
    ).map_err(|e| e.to_string())?;
    get_session(conn, id)?.ok_or_else(|| "updated session could not be read".into())
}

fn context(conn: &Connection, at: DateTime<FixedOffset>) -> AppResult<serde_json::Value> {
    let instant = at.to_rfc3339();
    let current = query_session(conn, "status='in_progress'", "actual_start_at DESC", None)?;
    let previous = query_session(conn, "status='completed' AND actual_end_at <= ?1", "actual_end_at DESC", Some(&instant))?;
    let next = query_session(conn, "status='planned' AND planned_start_at >= ?1", "planned_start_at ASC", Some(&instant))?;
    let until = next.as_ref().map(|s| minutes_between(&at, &parse_time(&s.planned_start_at).expect("stored timestamp must be valid")));
    let since = previous.as_ref().and_then(|s| s.actual_end_at.as_deref()).map(|v| minutes_between(&parse_time(v).expect("stored timestamp must be valid"), &at));
    Ok(json!({
        "at": instant,
        "timezone_offset_seconds": at.offset().local_minus_utc(),
        "current_session": current,
        "previous_session": previous,
        "next_session": next,
        "minutes_until_next_session": until,
        "minutes_since_previous_session": since,
        "data_completeness": {
            "has_current_session": current.is_some(),
            "has_previous_session": previous.is_some(),
            "has_next_session": next.is_some(),
            "next_has_duration": next.as_ref().is_some_and(|s| s.planned_duration_min.is_some()),
            "next_has_intensity": next.as_ref().is_some_and(|s| s.target_rpe.is_some())
        }
    }))
}

fn query_session(conn: &Connection, predicate: &str, order: &str, value: Option<&str>) -> AppResult<Option<Session>> {
    let sql = format!("SELECT id,title,type,focus_json,status,planned_start_at,planned_duration_min,actual_start_at,actual_end_at,target_rpe,workout_id,notes,created_at,updated_at FROM scheduled_sessions WHERE {predicate} ORDER BY {order} LIMIT 1");
    let result = match value {
        Some(v) => conn.query_row(&sql, params![v], map_session),
        None => conn.query_row(&sql, [], map_session),
    };
    result.optional().map_err(|e| e.to_string())
}

fn get_session(conn: &Connection, id: &str) -> AppResult<Option<Session>> {
    conn.query_row(
        "SELECT id,title,type,focus_json,status,planned_start_at,planned_duration_min,actual_start_at,actual_end_at,target_rpe,workout_id,notes,created_at,updated_at FROM scheduled_sessions WHERE id=?",
        params![id], map_session,
    ).optional().map_err(|e| e.to_string())
}

fn map_session(row: &Row<'_>) -> rusqlite::Result<Session> {
    let focus_json: String = row.get(3)?;
    Ok(Session {
        id: row.get(0)?, title: row.get(1)?, session_type: row.get(2)?,
        focus: serde_json::from_str(&focus_json).unwrap_or_default(), status: row.get(4)?,
        planned_start_at: row.get(5)?, planned_duration_min: row.get(6)?, actual_start_at: row.get(7)?,
        actual_end_at: row.get(8)?, target_rpe: row.get(9)?, workout_id: row.get(10)?, notes: row.get(11)?,
        created_at: row.get(12)?, updated_at: row.get(13)?,
    })
}

fn parse_time(value: &str) -> AppResult<DateTime<FixedOffset>> {
    DateTime::parse_from_rfc3339(value).map_err(|_| format!("invalid RFC 3339 timestamp: {value}"))
}

fn parse_or_now(value: Option<&str>) -> AppResult<DateTime<FixedOffset>> {
    value.map(parse_time).unwrap_or_else(|| Ok(Local::now().fixed_offset()))
}

fn validate_status(value: &str) -> AppResult<()> {
    match value {
        "planned" | "in_progress" | "completed" | "cancelled" => Ok(()),
        _ => Err(format!("invalid status: {value}")),
    }
}

fn minutes_between(from: &DateTime<FixedOffset>, to: &DateTime<FixedOffset>) -> i64 {
    to.signed_duration_since(*from).num_minutes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_offset_and_calculates_absolute_minutes() {
        let from = parse_time("2026-07-13T17:00:00+02:00").unwrap();
        let to = parse_time("2026-07-13T17:30:00+01:00").unwrap();
        assert_eq!(from.offset().local_minus_utc(), 7200);
        assert_eq!(minutes_between(&from, &to), 90);
    }

    #[test]
    fn rejects_unknown_status() {
        assert!(validate_status("delayed").is_err());
    }
}

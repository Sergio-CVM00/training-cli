use chrono::{Duration, Local, NaiveDate, SecondsFormat, Utc};
use clap::error::ErrorKind;
use clap::{Args, Parser, Subcommand};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use uuid::Uuid;

type AppResult<T> = Result<T, String>;

const DEFAULT_CONFIG: &str = r#"{
  "default_distance_unit": "km",
  "default_weight_unit": "kg",
  "default_workout_type": "gym",
  "training_constraints": [],
  "user_goal": ""
}
"#;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS workout_sessions (
  id TEXT PRIMARY KEY,
  date TEXT NOT NULL,
  title TEXT,
  type TEXT NOT NULL DEFAULT 'gym',
  bodyweight_kg REAL,
  duration_min INTEGER,
  distance_km REAL,
  speed_kmh REAL,
  pace_min_per_km REAL,
  elevation_gain_m REAL,
  avg_heart_rate_bpm INTEGER,
  max_heart_rate_bpm INTEGER,
  calories INTEGER,
  steps INTEGER,
  perceived_energy INTEGER,
  perceived_recovery INTEGER,
  notes TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS exercise_logs (
  id TEXT PRIMARY KEY,
  workout_id TEXT NOT NULL,
  exercise_name TEXT NOT NULL,
  category TEXT,
  muscle_group_json TEXT,
  equipment TEXT,
  notes TEXT,
  sort_order INTEGER NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (workout_id) REFERENCES workout_sessions(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS set_logs (
  id TEXT PRIMARY KEY,
  exercise_log_id TEXT NOT NULL,
  set_number INTEGER NOT NULL,
  set_type TEXT NOT NULL DEFAULT 'working',
  weight_kg REAL,
  reps INTEGER,
  target_reps INTEGER,
  rpe REAL,
  rir REAL,
  rest_sec INTEGER,
  tempo TEXT,
  form_rating INTEGER,
  pain_rating INTEGER,
  notes TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (exercise_log_id) REFERENCES exercise_logs(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS exercise_catalog (
  id TEXT PRIMARY KEY,
  source_id TEXT NOT NULL UNIQUE,
  name TEXT NOT NULL,
  category TEXT,
  body_part TEXT,
  equipment TEXT,
  target TEXT,
  muscle_group TEXT,
  secondary_muscles_json TEXT NOT NULL DEFAULT '[]',
  instructions_en TEXT,
  media_id TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_workout_sessions_date ON workout_sessions(date);
CREATE INDEX IF NOT EXISTS idx_exercise_logs_name ON exercise_logs(exercise_name);
CREATE INDEX IF NOT EXISTS idx_set_logs_exercise_log_id ON set_logs(exercise_log_id);
CREATE INDEX IF NOT EXISTS idx_exercise_catalog_name ON exercise_catalog(name);

CREATE TABLE IF NOT EXISTS command_receipts (
  command_id TEXT PRIMARY KEY,
  command_type TEXT NOT NULL,
  fingerprint TEXT NOT NULL,
  result_json TEXT NOT NULL,
  created_at TEXT NOT NULL
);
"#;

#[derive(Parser)]
#[command(name = "training", version, about = "Local-first workout logging CLI.")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Init { #[arg(long)] local: bool },
    Config { #[command(subcommand)] command: ConfigCommand },
    Add { #[command(subcommand)] command: AddCommand },
    List { #[command(subcommand)] command: ListCommand },
    Show { #[command(subcommand)] command: ShowCommand },
    Update { #[command(subcommand)] command: UpdateCommand },
    Delete { #[command(subcommand)] command: DeleteCommand },
    Exercises { #[command(subcommand)] command: ExercisesCommand },
    Log(LogArgs),
    Last { exercise: String, #[arg(long)] json: bool },
    History(HistoryArgs),
    Context { #[arg(long, default_value = "4weeks")] last: String, #[arg(long, default_value = "markdown")] format: String },
    Export { #[arg(long)] format: String, #[arg(long)] out: Option<PathBuf> },
}

#[derive(Subcommand)]
enum ConfigCommand {
    Get,
    Set { key: String, value: String },
}

#[derive(Subcommand)]
enum AddCommand {
    Workout(WorkoutArgs),
    Exercise(ExerciseArgs),
    Set(SetArgs),
}

#[derive(Subcommand)]
enum ListCommand {
    Workouts(RangeArgs),
}

#[derive(Subcommand)]
enum ShowCommand {
    Workout { identifier: String },
}

#[derive(Subcommand)]
enum UpdateCommand {
    Workout(UpdateWorkoutArgs),
    Exercise(UpdateExerciseArgs),
    Set(UpdateSetArgs),
}

#[derive(Subcommand)]
enum DeleteCommand {
    Workout { id: String, #[arg(long)] yes: bool },
    Exercise { id: String, #[arg(long)] yes: bool },
    Set { id: String, #[arg(long)] yes: bool },
}

#[derive(Subcommand)]
enum ExercisesCommand {
    Import(ExerciseImportArgs),
    Search(ExerciseSearchArgs),
    Show { query: String },
}

#[derive(Args)]
struct WorkoutArgs {
    #[arg(long = "date", default_value = "today")]
    date_value: String,
    #[arg(long)]
    title: Option<String>,
    #[arg(long, default_value = "gym")]
    r#type: String,
    #[arg(long)]
    bodyweight_kg: Option<f64>,
    #[arg(long)]
    duration_min: Option<i64>,
    #[arg(long)]
    distance_km: Option<f64>,
    #[arg(long)]
    speed_kmh: Option<f64>,
    #[arg(long)]
    pace_min_per_km: Option<f64>,
    #[arg(long)]
    elevation_gain_m: Option<f64>,
    #[arg(long)]
    avg_heart_rate_bpm: Option<i64>,
    #[arg(long)]
    max_heart_rate_bpm: Option<i64>,
    #[arg(long)]
    calories: Option<i64>,
    #[arg(long)]
    steps: Option<i64>,
    #[arg(long)]
    energy: Option<i64>,
    #[arg(long)]
    recovery: Option<i64>,
    #[arg(long)]
    notes: Option<String>,
}

#[derive(Args)]
struct ExerciseArgs {
    #[arg(long, default_value = "today")]
    workout: String,
    #[arg(long)]
    name: String,
    #[arg(long)]
    category: Option<String>,
    #[arg(long)]
    muscle_group: Option<String>,
    #[arg(long)]
    equipment: Option<String>,
    #[arg(long)]
    notes: Option<String>,
}

#[derive(Args)]
struct SetArgs {
    #[arg(long)]
    exercise: String,
    #[arg(long, default_value = "working")]
    set_type: String,
    #[arg(long)]
    weight: Option<f64>,
    #[arg(long)]
    reps: Option<i64>,
    #[arg(long)]
    target_reps: Option<i64>,
    #[arg(long)]
    rpe: Option<f64>,
    #[arg(long)]
    rir: Option<f64>,
    #[arg(long)]
    rest_sec: Option<i64>,
    #[arg(long)]
    tempo: Option<String>,
    #[arg(long)]
    form: Option<i64>,
    #[arg(long)]
    pain: Option<i64>,
    #[arg(long)]
    notes: Option<String>,
}

#[derive(Args)]
struct LogArgs {
    text: String,
    #[arg(long, default_value = "today")]
    workout: String,
    #[arg(long)]
    partial: bool,
    #[arg(long)]
    command_id: Option<String>,
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct ExerciseImportArgs {
    #[arg(long)]
    file: PathBuf,
}

#[derive(Args)]
struct ExerciseSearchArgs {
    query: String,
    #[arg(long, default_value_t = 10)]
    limit: i64,
}

#[derive(Args)]
struct RangeArgs {
    #[arg(long)]
    last: Option<String>,
    #[arg(long = "from")]
    from_date: Option<String>,
    #[arg(long = "to")]
    to_date: Option<String>,
}

#[derive(Args)]
struct HistoryArgs {
    exercise: String,
    #[arg(long)]
    last: Option<String>,
    #[arg(long = "from")]
    from_date: Option<String>,
    #[arg(long = "to")]
    to_date: Option<String>,
}

#[derive(Args)]
struct UpdateWorkoutArgs {
    id: String,
    #[arg(long = "date")]
    date_value: Option<String>,
    #[arg(long)]
    title: Option<String>,
    #[arg(long)]
    r#type: Option<String>,
    #[arg(long)]
    bodyweight_kg: Option<f64>,
    #[arg(long)]
    duration_min: Option<i64>,
    #[arg(long)]
    distance_km: Option<f64>,
    #[arg(long)]
    speed_kmh: Option<f64>,
    #[arg(long)]
    pace_min_per_km: Option<f64>,
    #[arg(long)]
    elevation_gain_m: Option<f64>,
    #[arg(long)]
    avg_heart_rate_bpm: Option<i64>,
    #[arg(long)]
    max_heart_rate_bpm: Option<i64>,
    #[arg(long)]
    calories: Option<i64>,
    #[arg(long)]
    steps: Option<i64>,
    #[arg(long)]
    energy: Option<i64>,
    #[arg(long)]
    recovery: Option<i64>,
    #[arg(long)]
    notes: Option<String>,
}

#[derive(Args)]
struct UpdateExerciseArgs {
    id: String,
    #[arg(long)]
    name: Option<String>,
    #[arg(long)]
    category: Option<String>,
    #[arg(long)]
    muscle_group: Option<String>,
    #[arg(long)]
    equipment: Option<String>,
    #[arg(long)]
    notes: Option<String>,
}

#[derive(Args)]
struct UpdateSetArgs {
    id: String,
    #[arg(long)]
    set_type: Option<String>,
    #[arg(long)]
    weight: Option<f64>,
    #[arg(long)]
    reps: Option<i64>,
    #[arg(long)]
    target_reps: Option<i64>,
    #[arg(long)]
    rpe: Option<f64>,
    #[arg(long)]
    rir: Option<f64>,
    #[arg(long)]
    rest_sec: Option<i64>,
    #[arg(long)]
    tempo: Option<String>,
    #[arg(long)]
    form: Option<i64>,
    #[arg(long)]
    pain: Option<i64>,
    #[arg(long)]
    notes: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct Workout {
    id: String,
    date: String,
    title: Option<String>,
    #[serde(rename = "type")]
    workout_type: String,
    bodyweight_kg: Option<f64>,
    duration_min: Option<i64>,
    distance_km: Option<f64>,
    speed_kmh: Option<f64>,
    pace_min_per_km: Option<f64>,
    elevation_gain_m: Option<f64>,
    avg_heart_rate_bpm: Option<i64>,
    max_heart_rate_bpm: Option<i64>,
    calories: Option<i64>,
    steps: Option<i64>,
    perceived_energy: Option<i64>,
    perceived_recovery: Option<i64>,
    notes: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
struct Exercise {
    id: String,
    workout_id: String,
    exercise_name: String,
    category: Option<String>,
    muscle_group_json: Option<String>,
    equipment: Option<String>,
    notes: Option<String>,
    sort_order: i64,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
struct SetLog {
    id: String,
    exercise_log_id: String,
    set_number: i64,
    set_type: String,
    weight_kg: Option<f64>,
    reps: Option<i64>,
    target_reps: Option<i64>,
    rpe: Option<f64>,
    rir: Option<f64>,
    rest_sec: Option<i64>,
    tempo: Option<String>,
    form_rating: Option<i64>,
    pain_rating: Option<i64>,
    notes: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
struct CatalogExercise {
    id: String,
    source_id: String,
    name: String,
    category: Option<String>,
    body_part: Option<String>,
    equipment: Option<String>,
    target: Option<String>,
    muscle_group: Option<String>,
    secondary_muscles_json: String,
    instructions_en: Option<String>,
    media_id: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct ImportedCatalogExercise {
    id: String,
    name: String,
    category: Option<String>,
    body_part: Option<String>,
    equipment: Option<String>,
    target: Option<String>,
    muscle_group: Option<String>,
    secondary_muscles: Option<Vec<String>>,
    instructions: Option<ImportedInstructions>,
    media_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ImportedInstructions {
    en: Option<String>,
}

#[derive(Clone)]
struct Paths {
    root: PathBuf,
    db: PathBuf,
    config: PathBuf,
    exports: PathBuf,
}

#[derive(Debug)]
struct ParsedSet {
    weight_kg: f64,
    reps: i64,
    rpe: Option<f64>,
    pain_rating: Option<i64>,
    form_rating: Option<i64>,
}

#[derive(Debug)]
struct ParsedExercise {
    name: String,
    sets: Vec<ParsedSet>,
}

struct CommandReceipt {
    command_type: String,
    fingerprint: String,
    result_json: String,
}

fn main() {
    let json_output = env::args().any(|argument| argument == "--json");
    if let Err(error) = run() {
        if json_output {
            let code = if error.contains("already used with different input") {
                "COMMAND_CONFLICT"
            } else if error.to_lowercase().contains("not found") {
                "NOT_FOUND"
            } else {
                "INVALID_COMMAND"
            };
            println!(
                "{}",
                serde_json::to_string(&json!({
                    "ok": false,
                    "error": {
                        "code": code,
                        "message": error,
                        "retryable": false,
                    }
                }))
                .unwrap_or_else(|_| "{\"ok\":false,\"error\":{\"code\":\"INTERNAL_ERROR\",\"message\":\"Could not serialize error\",\"retryable\":false}}".to_string())
            );
        } else {
            eprintln!("Error: {error}");
        }
        std::process::exit(1);
    }
}

fn run() -> AppResult<()> {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) if matches!(error.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) => {
            print!("{error}");
            return Ok(());
        }
        Err(error) => return Err(error.to_string()),
    };
    match cli.command {
        Commands::Init { local } => {
            let paths = init_storage(local)?;
            println!("Initialized training-cli");
            println!("Database: {}", paths.db.display());
            println!("Config: {}", paths.config.display());
            println!("Exports: {}", paths.exports.display());
        }
        Commands::Config { command } => handle_config(command)?,
        Commands::Add { command } => handle_add(command)?,
        Commands::List { command } => handle_list(command)?,
        Commands::Show { command } => handle_show(command)?,
        Commands::Update { command } => handle_update(command)?,
        Commands::Delete { command } => handle_delete(command)?,
        Commands::Exercises { command } => handle_exercises(command)?,
        Commands::Log(args) => handle_log(args)?,
        Commands::Last { exercise, json } => handle_last(&exercise, json)?,
        Commands::History(args) => handle_history(args)?,
        Commands::Context { last, format } => handle_context(&last, &format)?,
        Commands::Export { format, out } => handle_export(&format, out)?,
    }
    Ok(())
}

fn handle_config(command: ConfigCommand) -> AppResult<()> {
    let paths = ensure_ready()?;
    match command {
        ConfigCommand::Get => {
            println!("{}", serde_json::to_string_pretty(&read_config(&paths)?).map_err(json_err)?);
        }
        ConfigCommand::Set { key, value } => {
            let mut config = read_config(&paths)?;
            match key.as_str() {
                "goal" | "user_goal" => config["user_goal"] = Value::String(value),
                "constraint" | "knee_constraint" => {
                    let mut items = config["training_constraints"].as_array().cloned().unwrap_or_default();
                    items.push(Value::String(value));
                    config["training_constraints"] = Value::Array(items);
                }
                other => config[other] = Value::String(value),
            }
            write_config(&paths, &config)?;
            println!("Set {key}");
        }
    }
    Ok(())
}

fn handle_add(command: AddCommand) -> AppResult<()> {
    let paths = ensure_ready()?;
    let conn = open_db(&paths)?;
    match command {
        AddCommand::Workout(args) => {
            validate_workout_type(&args.r#type)?;
            validate_range(args.energy, 1, 5, "energy")?;
            validate_range(args.recovery, 1, 5, "recovery")?;
            validate_nonnegative_f64(args.bodyweight_kg, "bodyweight_kg")?;
            validate_nonnegative_i64(args.duration_min, "duration_min")?;
            validate_cardio_args(
                args.distance_km,
                args.speed_kmh,
                args.pace_min_per_km,
                args.elevation_gain_m,
                args.avg_heart_rate_bpm,
                args.max_heart_rate_bpm,
                args.calories,
                args.steps,
            )?;
            let workout = create_workout(&conn, &args)?;
            println!("Created workout {}", workout.id);
            println!("{}", format_workout_header(&workout));
        }
        AddCommand::Exercise(args) => {
            validate_category(args.category.as_deref())?;
            let workout = get_workout(&conn, &args.workout)?.ok_or_else(|| format!("Workout not found: {}", args.workout))?;
            let exercise = create_exercise(&conn, &workout.id, &args.name, args.category, args.muscle_group, args.equipment, args.notes)?;
            println!("Created exercise {}: {}", exercise.id, exercise.exercise_name);
        }
        AddCommand::Set(args) => {
            let exercise = resolve_set_exercise(&conn, &args.exercise)?;
            let set = create_set(&conn, &exercise.id, &set_args_to_input(args)?)?;
            println!("Created set {}: {}", set.id, format_set(&set));
        }
    }
    Ok(())
}

fn handle_list(command: ListCommand) -> AppResult<()> {
    let paths = ensure_ready()?;
    let conn = open_db(&paths)?;
    match command {
        ListCommand::Workouts(args) => {
            let workouts = list_workouts(&conn, args.last.as_deref(), args.from_date.as_deref(), args.to_date.as_deref())?;
            if workouts.is_empty() {
                println!("No workouts found.");
            }
            for workout in workouts {
                let title = workout.title.map(|t| format!(" - {t}")).unwrap_or_default();
                println!("{} {} {}{}", workout.date, workout.id, workout.workout_type, title);
            }
        }
    }
    Ok(())
}

fn handle_show(command: ShowCommand) -> AppResult<()> {
    let paths = ensure_ready()?;
    let conn = open_db(&paths)?;
    match command {
        ShowCommand::Workout { identifier } => {
            let workout = get_workout(&conn, &identifier)?.ok_or_else(|| format!("Workout not found: {identifier}"))?;
            println!("{}", format_workout_details(&conn, &workout)?);
        }
    }
    Ok(())
}

fn handle_update(command: UpdateCommand) -> AppResult<()> {
    let paths = ensure_ready()?;
    let conn = open_db(&paths)?;
    match command {
        UpdateCommand::Workout(args) => {
            let current = get_workout(&conn, &args.id)?.ok_or_else(|| format!("Workout not found: {}", args.id))?;
            let date = match args.date_value.as_deref() {
                Some(value) => resolve_date(value)?,
                None => current.date,
            };
            let workout_type = args.r#type.unwrap_or(current.workout_type);
            validate_workout_type(&workout_type)?;
            validate_range(args.energy.or(current.perceived_energy), 1, 5, "energy")?;
            validate_range(args.recovery.or(current.perceived_recovery), 1, 5, "recovery")?;
            validate_cardio_args(
                args.distance_km.or(current.distance_km),
                args.speed_kmh.or(current.speed_kmh),
                args.pace_min_per_km.or(current.pace_min_per_km),
                args.elevation_gain_m.or(current.elevation_gain_m),
                args.avg_heart_rate_bpm.or(current.avg_heart_rate_bpm),
                args.max_heart_rate_bpm.or(current.max_heart_rate_bpm),
                args.calories.or(current.calories),
                args.steps.or(current.steps),
            )?;
            conn.execute(
                "UPDATE workout_sessions SET date = ?, title = ?, type = ?, bodyweight_kg = ?, duration_min = ?, distance_km = ?, speed_kmh = ?, pace_min_per_km = ?, elevation_gain_m = ?, avg_heart_rate_bpm = ?, max_heart_rate_bpm = ?, calories = ?, steps = ?, perceived_energy = ?, perceived_recovery = ?, notes = ?, updated_at = ? WHERE id = ?",
                params![
                    date,
                    args.title.or(current.title),
                    workout_type,
                    args.bodyweight_kg.or(current.bodyweight_kg),
                    args.duration_min.or(current.duration_min),
                    args.distance_km.or(current.distance_km),
                    args.speed_kmh.or(current.speed_kmh),
                    args.pace_min_per_km.or(current.pace_min_per_km),
                    args.elevation_gain_m.or(current.elevation_gain_m),
                    args.avg_heart_rate_bpm.or(current.avg_heart_rate_bpm),
                    args.max_heart_rate_bpm.or(current.max_heart_rate_bpm),
                    args.calories.or(current.calories),
                    args.steps.or(current.steps),
                    args.energy.or(current.perceived_energy),
                    args.recovery.or(current.perceived_recovery),
                    args.notes.or(current.notes),
                    now_iso(),
                    args.id
                ],
            ).map_err(db_err)?;
            println!("Updated workout {}", args.id);
        }
        UpdateCommand::Exercise(args) => {
            let current = get_exercise_by_id(&conn, &args.id)?.ok_or_else(|| format!("Exercise not found: {}", args.id))?;
            validate_category(args.category.as_deref().or(current.category.as_deref()))?;
            let muscle = args.muscle_group.map(split_csv).unwrap_or_else(|| current.muscle_group_json.unwrap_or_else(|| "[]".to_string()));
            conn.execute(
                "UPDATE exercise_logs SET exercise_name = ?, category = ?, muscle_group_json = ?, equipment = ?, notes = ?, updated_at = ? WHERE id = ?",
                params![
                    args.name.unwrap_or(current.exercise_name),
                    args.category.or(current.category),
                    muscle,
                    args.equipment.or(current.equipment),
                    args.notes.or(current.notes),
                    now_iso(),
                    args.id
                ],
            ).map_err(db_err)?;
            println!("Updated exercise {}", args.id);
        }
        UpdateCommand::Set(args) => {
            let current = get_set(&conn, &args.id)?.ok_or_else(|| format!("Set not found: {}", args.id))?;
            let input = SetInput {
                set_type: args.set_type.unwrap_or(current.set_type),
                weight_kg: args.weight.or(current.weight_kg),
                reps: args.reps.or(current.reps),
                target_reps: args.target_reps.or(current.target_reps),
                rpe: args.rpe.or(current.rpe),
                rir: args.rir.or(current.rir),
                rest_sec: args.rest_sec.or(current.rest_sec),
                tempo: args.tempo.or(current.tempo),
                form_rating: args.form.or(current.form_rating),
                pain_rating: args.pain.or(current.pain_rating),
                notes: args.notes.or(current.notes),
            };
            validate_set_input(&input)?;
            conn.execute(
                "UPDATE set_logs SET set_type = ?, weight_kg = ?, reps = ?, target_reps = ?, rpe = ?, rir = ?, rest_sec = ?, tempo = ?, form_rating = ?, pain_rating = ?, notes = ?, updated_at = ? WHERE id = ?",
                params![input.set_type, input.weight_kg, input.reps, input.target_reps, input.rpe, input.rir, input.rest_sec, input.tempo, input.form_rating, input.pain_rating, input.notes, now_iso(), args.id],
            ).map_err(db_err)?;
            let updated = get_set(&conn, &args.id)?.unwrap();
            println!("Updated set {}: {}", args.id, format_set(&updated));
        }
    }
    Ok(())
}

fn handle_delete(command: DeleteCommand) -> AppResult<()> {
    let paths = ensure_ready()?;
    let conn = open_db(&paths)?;
    match command {
        DeleteCommand::Workout { id, yes } => {
            confirm(yes, &format!("Delete workout {id} and all linked exercises and sets?"))?;
            let changed = conn.execute("DELETE FROM workout_sessions WHERE id = ?", params![id]).map_err(db_err)?;
            if changed == 0 { return Err(format!("Workout not found: {id}")); }
            println!("Deleted workout {id}");
        }
        DeleteCommand::Exercise { id, yes } => {
            confirm(yes, &format!("Delete exercise {id} and its sets?"))?;
            let changed = conn.execute("DELETE FROM exercise_logs WHERE id = ?", params![id]).map_err(db_err)?;
            if changed == 0 { return Err(format!("Exercise not found: {id}")); }
            println!("Deleted exercise {id}");
        }
        DeleteCommand::Set { id, yes } => {
            confirm(yes, &format!("Delete set {id}?"))?;
            let set = get_set(&conn, &id)?.ok_or_else(|| format!("Set not found: {id}"))?;
            conn.execute("DELETE FROM set_logs WHERE id = ?", params![id]).map_err(db_err)?;
            renumber_sets(&conn, &set.exercise_log_id)?;
            println!("Deleted set {id}");
        }
    }
    Ok(())
}

fn handle_exercises(command: ExercisesCommand) -> AppResult<()> {
    let paths = ensure_ready()?;
    let mut conn = open_db(&paths)?;
    match command {
        ExercisesCommand::Import(args) => {
            let text = fs::read_to_string(&args.file).map_err(fs_err)?;
            let items: Vec<ImportedCatalogExercise> = serde_json::from_str(&text).map_err(json_err)?;
            let count = import_catalog_exercises(&mut conn, items)?;
            println!("Imported {count} catalog exercises");
        }
        ExercisesCommand::Search(args) => {
            let items = search_catalog_exercises(&conn, &args.query, args.limit)?;
            if items.is_empty() {
                println!("No catalog exercises found.");
            }
            for item in items {
                println!("{}", format_catalog_summary(&item));
            }
        }
        ExercisesCommand::Show { query } => {
            let item = find_catalog_exercise(&conn, &query)?
                .ok_or_else(|| format!("Catalog exercise not found: {query}"))?;
            println!("{}", format_catalog_details(&item));
        }
    }
    Ok(())
}

fn handle_log(args: LogArgs) -> AppResult<()> {
    let paths = ensure_ready()?;
    let mut conn = open_db(&paths)?;
    let (parsed, errors) = parse_log_text(&args.text);
    if !errors.is_empty() && !args.partial {
        return Err(format!("Log was not saved because it contains invalid input:\n{}", errors.join("\n")));
    }
    if parsed.is_empty() {
        let detail = if errors.is_empty() {
            "No valid sets found in log input.".to_string()
        } else {
            format!("No valid sets found in log input:\n{}", errors.join("\n"))
        };
        return Err(detail);
    }

    let fingerprint = serde_json::to_string(&json!({
        "command": "log",
        "text": args.text,
        "workout": args.workout,
        "partial": args.partial,
    }))
    .map_err(json_err)?;
    let tx = conn.transaction().map_err(db_err)?;
    if let Some(command_id) = args.command_id.as_deref() {
        if let Some(receipt) = get_command_receipt(&tx, command_id)? {
            if receipt.command_type != "training.log" || receipt.fingerprint != fingerprint {
                return Err(format!(
                    "command id {command_id} was already used with different input"
                ));
            }
            let stored: Value = serde_json::from_str(&receipt.result_json).map_err(json_err)?;
            let data = stored
                .get("data")
                .ok_or_else(|| "stored command receipt is missing data".to_string())?;
            let human = stored
                .get("human")
                .and_then(Value::as_str)
                .ok_or_else(|| "stored command receipt is missing human output".to_string())?;
            print_log_result(args.json, data, true, human)?;
            return Ok(());
        }
    }
    let workout = get_or_create_log_workout(&tx, &paths, &args.workout)?;
    let mut saved: Vec<(Exercise, Vec<SetLog>)> = Vec::new();
    for item in parsed {
        let exercise = match get_exercise_in_workout(&tx, &workout.id, &item.name)? {
            Some(exercise) => exercise,
            None => create_exercise_from_log_name(&tx, &workout.id, &item.name)?,
        };
        let mut sets = Vec::new();
        for parsed_set in item.sets {
            let input = SetInput {
                set_type: "working".to_string(),
                weight_kg: Some(parsed_set.weight_kg),
                reps: Some(parsed_set.reps),
                target_reps: None,
                rpe: parsed_set.rpe,
                rir: None,
                rest_sec: None,
                tempo: None,
                form_rating: parsed_set.form_rating,
                pain_rating: parsed_set.pain_rating,
                notes: None,
            };
            validate_set_input(&input)?;
            sets.push(create_set(&tx, &exercise.id, &input)?);
        }
        saved.push((exercise, sets));
    }
    let output = render_log_output(&workout, &saved);
    let result = json!({
        "workout": &workout,
        "exercises": saved
            .iter()
            .map(|(exercise, sets)| json!({ "exercise": exercise, "sets": sets }))
            .collect::<Vec<_>>(),
    });
    let result_json = serde_json::to_string(&json!({ "data": &result, "human": &output }))
        .map_err(json_err)?;
    if let Some(command_id) = args.command_id.as_deref() {
        tx.execute(
            "INSERT INTO command_receipts (command_id, command_type, fingerprint, result_json, created_at) VALUES (?, 'training.log', ?, ?, ?)",
            params![command_id, fingerprint, result_json, now_iso()],
        )
        .map_err(db_err)?;
    }
    tx.commit().map_err(db_err)?;
    print_log_result(args.json, &result, false, &output)?;
    for error in errors {
        eprintln!("Warning: {error}");
    }
    Ok(())
}

fn handle_last(name: &str, json_output: bool) -> AppResult<()> {
    let paths = ensure_ready()?;
    let conn = open_db(&paths)?;
    match last_exercise(&conn, name)? {
        LastResult::Found { exercise, workout, sets } => {
            if json_output {
                println!("{}", serde_json::to_string_pretty(&json!({"exercise": exercise, "workout": workout, "sets": sets})).map_err(json_err)?);
            } else {
                println!("{} - Last Session", exercise.exercise_name);
                println!("Date: {}", workout.date);
                println!("Workout: {}", workout.title.unwrap_or_else(|| "Untitled".to_string()));
                println!();
                for set in sets {
                    println!("{}. {}", set.set_number, format_set(&set));
                }
            }
        }
        LastResult::Matches(matches) => {
            println!("No exact match found for \"{name}\". Did you mean:");
            for (index, item) in matches.iter().enumerate() {
                println!("{}. {item}", index + 1);
            }
            return Err("no exact exercise match".to_string());
        }
        LastResult::None => return Err(format!("No history found for \"{name}\".")),
    }
    Ok(())
}

fn handle_history(args: HistoryArgs) -> AppResult<()> {
    let paths = ensure_ready()?;
    let conn = open_db(&paths)?;
    println!("{} - History\n", args.exercise);
    let items = history(&conn, &args.exercise, args.last.as_deref(), args.from_date.as_deref(), args.to_date.as_deref())?;
    if items.is_empty() {
        if !exercise_name_exists(&conn, &args.exercise)? {
            let matches = exercise_name_suggestions(&conn, &args.exercise)?;
            if !matches.is_empty() {
                println!("No exact match found for \"{}\". Did you mean:", args.exercise);
                for (index, item) in matches.iter().enumerate() {
                    println!("{}. {item}", index + 1);
                }
                return Err("no exact exercise match".to_string());
            }
        }
        println!("No history found.");
    }
    for (workout, _exercise, sets) in items {
        println!("{}", workout.date);
        let mut volume = 0.0;
        for set in sets {
            if let (Some(weight), Some(reps)) = (set.weight_kg, set.reps) {
                volume += weight * reps as f64;
            }
            println!("- {}", format_set(&set));
        }
        println!("Total working volume: {}kg\n", trim_float(volume));
    }
    Ok(())
}

fn handle_context(last: &str, format: &str) -> AppResult<()> {
    let paths = ensure_ready()?;
    let conn = open_db(&paths)?;
    let data = context_data(&conn, &paths, last)?;
    match format {
        "json" => println!("{}", serde_json::to_string_pretty(&data).map_err(json_err)?),
        "markdown" => println!("{}", context_markdown(&data)),
        _ => return Err("format must be markdown or json".to_string()),
    }
    Ok(())
}

fn handle_export(format: &str, out: Option<PathBuf>) -> AppResult<()> {
    if format != "json" {
        return Err("MVP supports json export".to_string());
    }
    let paths = ensure_ready()?;
    let conn = open_db(&paths)?;
    let target = out.unwrap_or_else(|| paths.exports.join(format!("training-export-{}.json", today())));
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(fs_err)?;
    }
    let data = json!({
        "workout_sessions": all_workouts(&conn)?,
        "exercise_logs": all_exercises(&conn)?,
        "set_logs": all_sets(&conn)?,
        "exercise_catalog": all_catalog_exercises(&conn)?,
    });
    fs::write(&target, serde_json::to_string_pretty(&data).map_err(json_err)? + "\n").map_err(fs_err)?;
    println!("Exported to {}", target.display());
    Ok(())
}

#[derive(Clone)]
struct SetInput {
    set_type: String,
    weight_kg: Option<f64>,
    reps: Option<i64>,
    target_reps: Option<i64>,
    rpe: Option<f64>,
    rir: Option<f64>,
    rest_sec: Option<i64>,
    tempo: Option<String>,
    form_rating: Option<i64>,
    pain_rating: Option<i64>,
    notes: Option<String>,
}

enum LastResult {
    Found { exercise: Exercise, workout: Workout, sets: Vec<SetLog> },
    Matches(Vec<String>),
    None,
}

fn paths(local: bool) -> AppResult<Paths> {
    let root = if local || env::var("TRAINING_CLI_LOCAL").ok().as_deref() == Some("1") {
        env::current_dir().map_err(fs_err)?.join(".training")
    } else if let Ok(home) = env::var("TRAINING_CLI_HOME") {
        PathBuf::from(home)
    } else if env::current_dir().map_err(fs_err)?.join(".training").exists() {
        env::current_dir().map_err(fs_err)?.join(".training")
    } else {
        PathBuf::from(env::var("HOME").map_err(|_| "HOME is not set".to_string())?).join(".training-cli")
    };
    Ok(Paths {
        db: root.join("training.db"),
        config: root.join("config.json"),
        exports: root.join("exports"),
        root,
    })
}

fn init_storage(local: bool) -> AppResult<Paths> {
    let paths = paths(local)?;
    fs::create_dir_all(&paths.root).map_err(fs_err)?;
    fs::create_dir_all(&paths.exports).map_err(fs_err)?;
    let conn = open_db(&paths)?;
    conn.execute_batch(SCHEMA).map_err(db_err)?;
    migrate_schema(&conn)?;
    if !paths.config.exists() {
        fs::write(&paths.config, DEFAULT_CONFIG).map_err(fs_err)?;
    }
    Ok(paths)
}

fn ensure_ready() -> AppResult<Paths> {
    let paths = paths(false)?;
    if !paths.db.exists() || !paths.config.exists() {
        init_storage(paths.root.file_name().and_then(|n| n.to_str()) == Some(".training"))
    } else {
        fs::create_dir_all(&paths.exports).map_err(fs_err)?;
        Ok(paths)
    }
}

fn open_db(paths: &Paths) -> AppResult<Connection> {
    fs::create_dir_all(&paths.root).map_err(fs_err)?;
    let conn = Connection::open(&paths.db).map_err(db_err)?;
    conn.pragma_update(None, "foreign_keys", "ON").map_err(db_err)?;
    if table_exists(&conn, "workout_sessions")? {
        migrate_schema(&conn)?;
    }
    Ok(conn)
}

fn migrate_schema(conn: &Connection) -> AppResult<()> {
    add_column_if_missing(conn, "workout_sessions", "distance_km", "REAL")?;
    add_column_if_missing(conn, "workout_sessions", "speed_kmh", "REAL")?;
    add_column_if_missing(conn, "workout_sessions", "pace_min_per_km", "REAL")?;
    add_column_if_missing(conn, "workout_sessions", "elevation_gain_m", "REAL")?;
    add_column_if_missing(conn, "workout_sessions", "avg_heart_rate_bpm", "INTEGER")?;
    add_column_if_missing(conn, "workout_sessions", "max_heart_rate_bpm", "INTEGER")?;
    add_column_if_missing(conn, "workout_sessions", "calories", "INTEGER")?;
    add_column_if_missing(conn, "workout_sessions", "steps", "INTEGER")?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS exercise_catalog (
          id TEXT PRIMARY KEY,
          source_id TEXT NOT NULL UNIQUE,
          name TEXT NOT NULL,
          category TEXT,
          body_part TEXT,
          equipment TEXT,
          target TEXT,
          muscle_group TEXT,
          secondary_muscles_json TEXT NOT NULL DEFAULT '[]',
          instructions_en TEXT,
          media_id TEXT,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_exercise_catalog_name ON exercise_catalog(name);",
    )
    .map_err(db_err)?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS command_receipts (
          command_id TEXT PRIMARY KEY,
          command_type TEXT NOT NULL,
          fingerprint TEXT NOT NULL,
          result_json TEXT NOT NULL,
          created_at TEXT NOT NULL
        );",
    )
    .map_err(db_err)?;
    Ok(())
}

fn table_exists(conn: &Connection, table: &str) -> AppResult<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?)",
        params![table],
        |row| row.get::<_, i64>(0),
    )
    .map(|value| value == 1)
    .map_err(db_err)
}

fn add_column_if_missing(conn: &Connection, table: &str, column: &str, kind: &str) -> AppResult<()> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})")).map_err(db_err)?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(db_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(db_err)?;
    if !columns.iter().any(|existing| existing == column) {
        conn.execute(&format!("ALTER TABLE {table} ADD COLUMN {column} {kind}"), [])
            .map_err(db_err)?;
    }
    Ok(())
}

fn read_config(paths: &Paths) -> AppResult<Value> {
    let text = fs::read_to_string(&paths.config)
        .map_err(|error| format!("could not read {}: {error}", paths.config.display()))?;
    let mut value: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    for (key, default) in serde_json::from_str::<Value>(DEFAULT_CONFIG).unwrap().as_object().unwrap() {
        if value.get(key).is_none() {
            value[key] = default.clone();
        }
    }
    Ok(value)
}

fn write_config(paths: &Paths, config: &Value) -> AppResult<()> {
    fs::write(&paths.config, serde_json::to_string_pretty(config).map_err(json_err)? + "\n").map_err(fs_err)
}

fn create_workout(conn: &Connection, args: &WorkoutArgs) -> AppResult<Workout> {
    let id = Uuid::new_v4().to_string();
    let now = now_iso();
    let date = resolve_date(&args.date_value)?;
    conn.execute(
        "INSERT INTO workout_sessions (id, date, title, type, bodyweight_kg, duration_min, distance_km, speed_kmh, pace_min_per_km, elevation_gain_m, avg_heart_rate_bpm, max_heart_rate_bpm, calories, steps, perceived_energy, perceived_recovery, notes, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![id, date, args.title, args.r#type, args.bodyweight_kg, args.duration_min, args.distance_km, args.speed_kmh, args.pace_min_per_km, args.elevation_gain_m, args.avg_heart_rate_bpm, args.max_heart_rate_bpm, args.calories, args.steps, args.energy, args.recovery, args.notes, now, now],
    ).map_err(db_err)?;
    get_workout(conn, &id)?.ok_or_else(|| "created workout could not be read".to_string())
}

fn create_exercise(conn: &Connection, workout_id: &str, name: &str, category: Option<String>, muscle_group: Option<String>, equipment: Option<String>, notes: Option<String>) -> AppResult<Exercise> {
    if name.trim().is_empty() {
        return Err("exercise name is required".to_string());
    }
    let id = Uuid::new_v4().to_string();
    let now = now_iso();
    let sort_order: i64 = conn.query_row("SELECT COALESCE(MAX(sort_order), 0) + 1 FROM exercise_logs WHERE workout_id = ?", params![workout_id], |r| r.get(0)).map_err(db_err)?;
    let muscle_group_json = muscle_group.map(split_csv).unwrap_or_else(|| "[]".to_string());
    conn.execute(
        "INSERT INTO exercise_logs (id, workout_id, exercise_name, category, muscle_group_json, equipment, notes, sort_order, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![id, workout_id, name.trim(), category, muscle_group_json, equipment, notes, sort_order, now, now],
    ).map_err(db_err)?;
    get_exercise_by_id(conn, &id)?.ok_or_else(|| "created exercise could not be read".to_string())
}

fn create_exercise_from_log_name(conn: &Connection, workout_id: &str, name: &str) -> AppResult<Exercise> {
    if let Some(catalog) = find_catalog_exercise(conn, name)? {
        let category = catalog_category_to_log_category(catalog.category.as_deref().or(catalog.body_part.as_deref()));
        let muscle_group = catalog_muscle_group(&catalog);
        return create_exercise(
            conn,
            workout_id,
            &catalog.name,
            category,
            muscle_group,
            catalog.equipment,
            None,
        );
    }
    create_exercise(conn, workout_id, name, None, None, None, None)
}

fn create_set(conn: &Connection, exercise_id: &str, input: &SetInput) -> AppResult<SetLog> {
    validate_set_input(input)?;
    let id = Uuid::new_v4().to_string();
    let now = now_iso();
    let set_number: i64 = conn.query_row("SELECT COALESCE(MAX(set_number), 0) + 1 FROM set_logs WHERE exercise_log_id = ?", params![exercise_id], |r| r.get(0)).map_err(db_err)?;
    conn.execute(
        "INSERT INTO set_logs (id, exercise_log_id, set_number, set_type, weight_kg, reps, target_reps, rpe, rir, rest_sec, tempo, form_rating, pain_rating, notes, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![id, exercise_id, set_number, input.set_type, input.weight_kg, input.reps, input.target_reps, input.rpe, input.rir, input.rest_sec, input.tempo, input.form_rating, input.pain_rating, input.notes, now, now],
    ).map_err(db_err)?;
    get_set(conn, &id)?.ok_or_else(|| "created set could not be read".to_string())
}

fn get_or_create_today_workout(conn: &Connection, paths: &Paths) -> AppResult<Workout> {
    if let Some(workout) = get_workout(conn, "today")? {
        return Ok(workout);
    }
    let config = read_config(paths)?;
    let workout_type = config["default_workout_type"].as_str().unwrap_or("gym").to_string();
    create_workout(conn, &WorkoutArgs {
        date_value: "today".to_string(),
        title: None,
        r#type: workout_type,
        bodyweight_kg: None,
        duration_min: None,
        distance_km: None,
        speed_kmh: None,
        pace_min_per_km: None,
        elevation_gain_m: None,
        avg_heart_rate_bpm: None,
        max_heart_rate_bpm: None,
        calories: None,
        steps: None,
        energy: None,
        recovery: None,
        notes: None,
    })
}

fn get_or_create_log_workout(conn: &Connection, paths: &Paths, identifier: &str) -> AppResult<Workout> {
    if identifier == "today" {
        return get_or_create_today_workout(conn, paths);
    }
    get_workout(conn, identifier)?.ok_or_else(|| format!("Workout not found: {identifier}"))
}

fn get_workout(conn: &Connection, identifier: &str) -> AppResult<Option<Workout>> {
    if identifier == "today" {
        return conn.query_row("SELECT * FROM workout_sessions WHERE date = ? ORDER BY created_at DESC, id DESC LIMIT 1", params![today()], workout_from_row).optional().map_err(db_err);
    }
    conn.query_row("SELECT * FROM workout_sessions WHERE id = ?", params![identifier], workout_from_row).optional().map_err(db_err)
}

fn list_workouts(conn: &Connection, last: Option<&str>, from_date: Option<&str>, to_date: Option<&str>) -> AppResult<Vec<Workout>> {
    let (start, end) = parse_range(last, from_date, to_date)?;
    let mut sql = "SELECT * FROM workout_sessions".to_string();
    let mut clauses = Vec::new();
    if start.is_some() { clauses.push("date >= ?"); }
    if end.is_some() { clauses.push("date <= ?"); }
    if !clauses.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&clauses.join(" AND "));
    }
    sql.push_str(" ORDER BY date DESC, created_at DESC");
    let mut stmt = conn.prepare(&sql).map_err(db_err)?;
    let rows: Result<Vec<Workout>, rusqlite::Error> = match (start, end) {
        (Some(s), Some(e)) => stmt.query_map(params![s, e], workout_from_row).map_err(db_err)?.collect(),
        (Some(s), None) => stmt.query_map(params![s], workout_from_row).map_err(db_err)?.collect(),
        (None, Some(e)) => stmt.query_map(params![e], workout_from_row).map_err(db_err)?.collect(),
        (None, None) => stmt.query_map([], workout_from_row).map_err(db_err)?.collect(),
    };
    rows.map_err(db_err)
}

fn get_exercise_by_id(conn: &Connection, id: &str) -> AppResult<Option<Exercise>> {
    conn.query_row("SELECT * FROM exercise_logs WHERE id = ?", params![id], exercise_from_row).optional().map_err(db_err)
}

fn resolve_set_exercise(conn: &Connection, identifier: &str) -> AppResult<Exercise> {
    if let Some(exercise) = get_exercise_by_id(conn, identifier)? {
        return Ok(exercise);
    }
    let matches = today_exercise_matches(conn, identifier)?;
    match matches.len() {
        0 => Err(format!("Exercise \"{identifier}\" was not found in today's workout.")),
        1 => Ok(matches.into_iter().next().unwrap()),
        _ => Err(format!(
            "Exercise \"{identifier}\" is ambiguous today. Pass an exercise id instead."
        )),
    }
}

fn today_exercise_matches(conn: &Connection, name: &str) -> AppResult<Vec<Exercise>> {
    let mut stmt = conn.prepare(
        "SELECT e.* FROM exercise_logs e JOIN workout_sessions w ON w.id = e.workout_id WHERE w.date = ? AND lower(e.exercise_name) = lower(?) ORDER BY w.created_at DESC, e.sort_order DESC",
    ).map_err(db_err)?;
    stmt.query_map(params![today(), name], exercise_from_row)
        .map_err(db_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(db_err)
}

fn get_exercise_in_workout(conn: &Connection, workout_id: &str, name: &str) -> AppResult<Option<Exercise>> {
    conn.query_row(
        "SELECT * FROM exercise_logs WHERE workout_id = ? AND lower(exercise_name) = lower(?) ORDER BY sort_order DESC LIMIT 1",
        params![workout_id, name],
        exercise_from_row,
    ).optional().map_err(db_err)
}

fn get_set(conn: &Connection, id: &str) -> AppResult<Option<SetLog>> {
    conn.query_row("SELECT * FROM set_logs WHERE id = ?", params![id], set_from_row).optional().map_err(db_err)
}

fn get_command_receipt(conn: &Connection, command_id: &str) -> AppResult<Option<CommandReceipt>> {
    conn.query_row(
        "SELECT command_type, fingerprint, result_json FROM command_receipts WHERE command_id = ?",
        params![command_id],
        |row| {
            Ok(CommandReceipt {
                command_type: row.get(0)?,
                fingerprint: row.get(1)?,
                result_json: row.get(2)?,
            })
        },
    )
    .optional()
    .map_err(db_err)
}

fn render_log_output(workout: &Workout, saved: &[(Exercise, Vec<SetLog>)]) -> String {
    let mut output = format!("Saved workout log\n\n{}\n\n", format_workout_header(workout));
    for (exercise, sets) in saved {
        output.push_str(&exercise.exercise_name);
        output.push('\n');
        for set in sets {
            output.push_str(&format!("{}. {}\n", set.set_number, format_set(set)));
        }
        output.push('\n');
    }
    output
}

fn print_log_result(json_output: bool, data: &Value, replayed: bool, human: &str) -> AppResult<()> {
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "ok": true,
                "data": data,
                "meta": { "replayed": replayed },
            }))
            .map_err(json_err)?
        );
    } else {
        print!("{human}");
    }
    Ok(())
}

fn sets_for_exercise(conn: &Connection, exercise_id: &str) -> AppResult<Vec<SetLog>> {
    let mut stmt = conn.prepare("SELECT * FROM set_logs WHERE exercise_log_id = ? ORDER BY set_number").map_err(db_err)?;
    stmt.query_map(params![exercise_id], set_from_row).map_err(db_err)?.collect::<Result<Vec<_>, _>>().map_err(db_err)
}

fn exercises_for_workout(conn: &Connection, workout_id: &str) -> AppResult<Vec<Exercise>> {
    let mut stmt = conn.prepare("SELECT * FROM exercise_logs WHERE workout_id = ? ORDER BY sort_order").map_err(db_err)?;
    stmt.query_map(params![workout_id], exercise_from_row).map_err(db_err)?.collect::<Result<Vec<_>, _>>().map_err(db_err)
}

fn all_workouts(conn: &Connection) -> AppResult<Vec<Workout>> {
    let mut stmt = conn.prepare("SELECT * FROM workout_sessions ORDER BY date, created_at").map_err(db_err)?;
    stmt.query_map([], workout_from_row).map_err(db_err)?.collect::<Result<Vec<_>, _>>().map_err(db_err)
}

fn all_exercises(conn: &Connection) -> AppResult<Vec<Exercise>> {
    let mut stmt = conn.prepare("SELECT * FROM exercise_logs ORDER BY workout_id, sort_order").map_err(db_err)?;
    stmt.query_map([], exercise_from_row).map_err(db_err)?.collect::<Result<Vec<_>, _>>().map_err(db_err)
}

fn all_sets(conn: &Connection) -> AppResult<Vec<SetLog>> {
    let mut stmt = conn.prepare("SELECT * FROM set_logs ORDER BY exercise_log_id, set_number").map_err(db_err)?;
    stmt.query_map([], set_from_row).map_err(db_err)?.collect::<Result<Vec<_>, _>>().map_err(db_err)
}

fn import_catalog_exercises(conn: &mut Connection, items: Vec<ImportedCatalogExercise>) -> AppResult<usize> {
    let tx = conn.transaction().map_err(db_err)?;
    let now = now_iso();
    let mut count = 0;
    for item in items {
        if item.id.trim().is_empty() || item.name.trim().is_empty() {
            continue;
        }
        let secondary_muscles = serde_json::to_string(&item.secondary_muscles.unwrap_or_default()).map_err(json_err)?;
        let id = Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO exercise_catalog (id, source_id, name, category, body_part, equipment, target, muscle_group, secondary_muscles_json, instructions_en, media_id, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(source_id) DO UPDATE SET
               name = excluded.name,
               category = excluded.category,
               body_part = excluded.body_part,
               equipment = excluded.equipment,
               target = excluded.target,
               muscle_group = excluded.muscle_group,
               secondary_muscles_json = excluded.secondary_muscles_json,
               instructions_en = excluded.instructions_en,
               media_id = excluded.media_id,
               updated_at = excluded.updated_at",
            params![
                id,
                item.id.trim(),
                item.name.trim(),
                item.category,
                item.body_part,
                item.equipment,
                item.target,
                item.muscle_group,
                secondary_muscles,
                item.instructions.and_then(|instructions| instructions.en),
                item.media_id,
                now,
                now
            ],
        )
        .map_err(db_err)?;
        count += 1;
    }
    tx.commit().map_err(db_err)?;
    Ok(count)
}

fn all_catalog_exercises(conn: &Connection) -> AppResult<Vec<CatalogExercise>> {
    let mut stmt = conn.prepare("SELECT * FROM exercise_catalog ORDER BY name").map_err(db_err)?;
    stmt.query_map([], catalog_exercise_from_row)
        .map_err(db_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(db_err)
}

fn escape_like_pattern(value: &str) -> String {
    value.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

fn search_catalog_exercises(conn: &Connection, query: &str, limit: i64) -> AppResult<Vec<CatalogExercise>> {
    let pattern = format!("%{}%", escape_like_pattern(&query.trim().to_lowercase()));
    let mut stmt = conn
        .prepare(
            "SELECT * FROM exercise_catalog
             WHERE lower(name) LIKE ?1 ESCAPE '\\' OR lower(category) LIKE ?1 ESCAPE '\\' OR lower(equipment) LIKE ?1 ESCAPE '\\' OR lower(target) LIKE ?1 ESCAPE '\\'
             ORDER BY name, source_id
             LIMIT ?2",
        )
        .map_err(db_err)?;
    stmt.query_map(params![pattern, limit.max(1)], catalog_exercise_from_row)
        .map_err(db_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(db_err)
}

fn find_catalog_exercise(conn: &Connection, query: &str) -> AppResult<Option<CatalogExercise>> {
    conn.query_row(
        "SELECT * FROM exercise_catalog WHERE source_id = ?1 OR lower(name) = lower(?1) ORDER BY (source_id = ?1) DESC, name, source_id LIMIT 1",
        params![query],
        catalog_exercise_from_row,
    )
    .optional()
    .map_err(db_err)
}

fn catalog_category_to_log_category(value: Option<&str>) -> Option<String> {
    let value = value?.trim().to_lowercase();
    let category = match value.as_str() {
        "chest" | "shoulders" => "push",
        "back" | "lower arms" => "pull",
        "upper legs" | "lower legs" => "legs",
        "waist" => "core",
        "cardio" => "cardio",
        "mobility" => "mobility",
        _ => "other",
    };
    Some(category.to_string())
}

fn catalog_muscle_group(catalog: &CatalogExercise) -> Option<String> {
    let mut items = Vec::new();
    for value in [&catalog.target, &catalog.muscle_group].into_iter().flatten() {
        push_unique(&mut items, value);
    }
    if let Ok(values) = serde_json::from_str::<Vec<String>>(&catalog.secondary_muscles_json) {
        for value in values {
            push_unique(&mut items, &value);
        }
    }
    if items.is_empty() {
        None
    } else {
        Some(items.join(","))
    }
}

fn push_unique(items: &mut Vec<String>, value: &str) {
    let value = value.trim();
    if !value.is_empty() && !items.iter().any(|item| item.eq_ignore_ascii_case(value)) {
        items.push(value.to_string());
    }
}

fn exercise_name_exists(conn: &Connection, name: &str) -> AppResult<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM exercise_logs WHERE lower(exercise_name) = lower(?))",
        params![name],
        |row| row.get::<_, bool>(0),
    )
    .map_err(db_err)
}

fn exercise_name_suggestions(conn: &Connection, name: &str) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare("SELECT DISTINCT exercise_name FROM exercise_logs ORDER BY exercise_name").map_err(db_err)?;
    let names = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(db_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(db_err)?;
    let lowered = name.to_lowercase();
    Ok(names.into_iter().filter(|candidate| candidate.to_lowercase().contains(&lowered)).collect())
}

fn last_exercise(conn: &Connection, name: &str) -> AppResult<LastResult> {
    let mut stmt = conn.prepare(
        "SELECT e.*, w.id, w.date, w.title, w.type, w.bodyweight_kg, w.duration_min, w.distance_km, w.speed_kmh, w.pace_min_per_km, w.elevation_gain_m, w.avg_heart_rate_bpm, w.max_heart_rate_bpm, w.calories, w.steps, w.perceived_energy, w.perceived_recovery, w.notes, w.created_at, w.updated_at
         FROM exercise_logs e JOIN workout_sessions w ON w.id = e.workout_id
         ORDER BY w.date DESC, w.created_at DESC, e.sort_order DESC"
    ).map_err(db_err)?;
    let rows = stmt.query_map([], |row| Ok((exercise_from_join_row(row)?, workout_from_join_row(row)?))).map_err(db_err)?.collect::<Result<Vec<_>, _>>().map_err(db_err)?;
    for (exercise, workout) in &rows {
        if exercise.exercise_name.eq_ignore_ascii_case(name) {
            return Ok(LastResult::Found { exercise: exercise.clone(), workout: workout.clone(), sets: sets_for_exercise(conn, &exercise.id)? });
        }
    }
    let matches = exercise_name_suggestions(conn, name)?;
    if matches.is_empty() { Ok(LastResult::None) } else { Ok(LastResult::Matches(matches)) }
}

fn history(conn: &Connection, name: &str, last: Option<&str>, from_date: Option<&str>, to_date: Option<&str>) -> AppResult<Vec<(Workout, Exercise, Vec<SetLog>)>> {
    let (start, end) = parse_range(last, from_date, to_date)?;
    let mut sql = "SELECT e.*, w.id, w.date, w.title, w.type, w.bodyweight_kg, w.duration_min, w.distance_km, w.speed_kmh, w.pace_min_per_km, w.elevation_gain_m, w.avg_heart_rate_bpm, w.max_heart_rate_bpm, w.calories, w.steps, w.perceived_energy, w.perceived_recovery, w.notes, w.created_at, w.updated_at FROM exercise_logs e JOIN workout_sessions w ON w.id = e.workout_id WHERE lower(e.exercise_name) = lower(?)".to_string();
    if start.is_some() { sql.push_str(" AND w.date >= ?"); }
    if end.is_some() { sql.push_str(" AND w.date <= ?"); }
    sql.push_str(" ORDER BY w.date DESC, w.created_at DESC, e.sort_order");
    let mut stmt = conn.prepare(&sql).map_err(db_err)?;
    let pairs: Vec<(Exercise, Workout)> = match (start, end) {
        (Some(s), Some(e)) => stmt.query_map(params![name, s, e], |row| Ok((exercise_from_join_row(row)?, workout_from_join_row(row)?))).map_err(db_err)?.collect::<Result<Vec<_>, _>>(),
        (Some(s), None) => stmt.query_map(params![name, s], |row| Ok((exercise_from_join_row(row)?, workout_from_join_row(row)?))).map_err(db_err)?.collect::<Result<Vec<_>, _>>(),
        (None, Some(e)) => stmt.query_map(params![name, e], |row| Ok((exercise_from_join_row(row)?, workout_from_join_row(row)?))).map_err(db_err)?.collect::<Result<Vec<_>, _>>(),
        (None, None) => stmt.query_map(params![name], |row| Ok((exercise_from_join_row(row)?, workout_from_join_row(row)?))).map_err(db_err)?.collect::<Result<Vec<_>, _>>(),
    }.map_err(db_err)?;
    pairs.into_iter().map(|(exercise, workout)| {
        let sets = sets_for_exercise(conn, &exercise.id)?;
        Ok((workout, exercise, sets))
    }).collect()
}

fn context_data(conn: &Connection, paths: &Paths, last: &str) -> AppResult<Value> {
    let config = read_config(paths)?;
    let (start, end) = parse_range(Some(last), None, None)?;
    let workouts = list_workouts(conn, None, start.as_deref(), end.as_deref())?;
    let mut recent_sessions = Vec::new();
    let mut trends: BTreeMap<String, Value> = BTreeMap::new();
    let mut pain_notes = Vec::new();
    let mut form_notes = Vec::new();
    let mut data_quality_notes = Vec::new();
    for workout in workouts {
        let exercises = exercises_for_workout(conn, &workout.id)?;
        let mut exercise_values = Vec::new();
        for exercise in exercises {
            let sets = sets_for_exercise(conn, &exercise.id)?;
            let trend = trends.entry(exercise.exercise_name.clone()).or_insert(json!({"exercise_name": exercise.exercise_name, "sessions": 0, "sets": 0, "volume": 0.0}));
            trend["sessions"] = json!(trend["sessions"].as_i64().unwrap_or(0) + 1);
            trend["sets"] = json!(trend["sets"].as_i64().unwrap_or(0) + sets.len() as i64);
            for set in &sets {
                if let (Some(weight), Some(reps)) = (set.weight_kg, set.reps) {
                    trend["volume"] = json!(trend["volume"].as_f64().unwrap_or(0.0) + weight * reps as f64);
                }
                if set.pain_rating.unwrap_or(0) > 0 {
                    pain_notes.push(json!({"date": workout.date, "exercise": exercise.exercise_name, "set_number": set.set_number, "pain_rating": set.pain_rating}));
                }
                if set.form_rating.is_some_and(|value| value < 3) {
                    form_notes.push(json!({"date": workout.date, "exercise": exercise.exercise_name, "set_number": set.set_number, "form_rating": set.form_rating}));
                }
                let mut missing = Vec::new();
                if set.weight_kg.is_none() { missing.push("weight_kg"); }
                if set.reps.is_none() { missing.push("reps"); }
                if set.rpe.is_none() { missing.push("rpe"); }
                if !missing.is_empty() {
                    data_quality_notes.push(json!({"date": workout.date, "exercise": exercise.exercise_name, "set_number": set.set_number, "missing": missing}));
                }
            }
            exercise_values.push(json!({"exercise": exercise, "sets": sets}));
        }
        recent_sessions.push(json!({"workout": workout, "exercises": exercise_values}));
    }
    Ok(json!({
        "goal": config["user_goal"],
        "constraints": config["training_constraints"],
        "date_range": {"from": start, "to": end},
        "recent_sessions": recent_sessions,
        "exercise_trends": trends.values().cloned().collect::<Vec<_>>(),
        "pain_notes": pain_notes,
        "form_notes": form_notes,
        "data_quality_notes": data_quality_notes,
    }))
}

fn context_markdown(data: &Value) -> String {
    let mut lines = vec![
        "# Training Context".to_string(),
        "".to_string(),
        "## Goal".to_string(),
        data["goal"].as_str().filter(|s| !s.is_empty()).unwrap_or("Not set.").to_string(),
        "".to_string(),
        "## Constraints".to_string(),
    ];
    if let Some(items) = data["constraints"].as_array().filter(|items| !items.is_empty()) {
        for item in items {
            lines.push(format!("- {}", item.as_str().unwrap_or("")));
        }
    } else {
        lines.push("- None logged.".to_string());
    }
    lines.push("".to_string());
    lines.push("## Recent Sessions".to_string());
    if let Some(sessions) = data["recent_sessions"].as_array().filter(|items| !items.is_empty()) {
        for session in sessions {
            let workout = &session["workout"];
            lines.push(format!("### {} - {}", workout["date"].as_str().unwrap_or(""), workout["title"].as_str().unwrap_or("Untitled")));
            if let Some(cardio) = format_cardio_value(workout) {
                lines.push(format!("- Cardio: {cardio}"));
            }
            for exercise in session["exercises"].as_array().unwrap_or(&Vec::new()) {
                lines.push(format!("- {}", exercise["exercise"]["exercise_name"].as_str().unwrap_or("")));
                for set in exercise["sets"].as_array().unwrap_or(&Vec::new()) {
                    lines.push(format!("  - Set {}: {}", set["set_number"], format_set_value(set)));
                }
            }
        }
    } else {
        lines.push("No recent sessions.".to_string());
    }
    lines.push("".to_string());
    lines.push("## Exercise Trends".to_string());
    if let Some(trends) = data["exercise_trends"].as_array().filter(|items| !items.is_empty()) {
        for trend in trends {
            lines.push(format!("- {}: {} sessions, {} sets, {}kg volume", trend["exercise_name"].as_str().unwrap_or(""), trend["sessions"], trend["sets"], trim_float(trend["volume"].as_f64().unwrap_or(0.0))));
        }
    } else {
        lines.push("- No exercise trends available.".to_string());
    }
    lines.push("".to_string());
    lines.push("## Pain / Form Notes".to_string());
    let mut any_note = false;
    for note in data["pain_notes"].as_array().unwrap_or(&Vec::new()) {
        any_note = true;
        lines.push(format!("- Pain {} on {} for {} set {}.", note["pain_rating"], note["date"].as_str().unwrap_or(""), note["exercise"].as_str().unwrap_or(""), note["set_number"]));
    }
    for note in data["form_notes"].as_array().unwrap_or(&Vec::new()) {
        any_note = true;
        lines.push(format!("- Form {} on {} for {} set {}.", note["form_rating"], note["date"].as_str().unwrap_or(""), note["exercise"].as_str().unwrap_or(""), note["set_number"]));
    }
    if !any_note {
        lines.push("- No pain or low-form notes.".to_string());
    }
    lines.push("".to_string());
    lines.push("## Data Quality Notes".to_string());
    if let Some(notes) = data["data_quality_notes"].as_array().filter(|items| !items.is_empty()) {
        for note in notes {
            let missing = note["missing"].as_array().unwrap_or(&Vec::new()).iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(", ");
            lines.push(format!("- {} {} set {} missing {}.", note["date"].as_str().unwrap_or(""), note["exercise"].as_str().unwrap_or(""), note["set_number"], missing));
        }
    } else {
        lines.push("- No missing set data detected.".to_string());
    }
    lines.join("\n")
}

fn parse_log_text(text: &str) -> (Vec<ParsedExercise>, Vec<String>) {
    let mut exercises = Vec::new();
    let mut errors = Vec::new();
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() { continue; }
        let Some((raw_name, raw_sets)) = line.split_once(':') else {
            errors.push(format!("Invalid line: {raw_line}"));
            continue;
        };
        let name = raw_name.trim();
        if name.is_empty() {
            errors.push(format!("Invalid line: {raw_line}"));
            continue;
        }
        let mut sets = Vec::new();
        for raw_set in raw_sets.split(',') {
            let set_text = raw_set.trim();
            if set_text.is_empty() { continue; }
            match parse_set(set_text) {
                Ok(set) => sets.push(set),
                Err(error) => errors.push(format!("Invalid set for {name}: {set_text} ({error})")),
            }
        }
        if !sets.is_empty() {
            exercises.push(ParsedExercise { name: name.to_string(), sets });
        }
    }
    (exercises, errors)
}

fn parse_set(input: &str) -> AppResult<ParsedSet> {
    let mut tokens = input.split_whitespace();
    let core = tokens.next().ok_or_else(|| "missing set".to_string())?;
    let (weight_part, rest) = core.split_once(['x', 'X']).ok_or_else(|| "expected weightxreps".to_string())?;
    let (reps_part, inline_rpe) = match rest.split_once('@') {
        Some((reps, rpe)) => (reps, Some(rpe)),
        None => (rest, None),
    };
    let mut rpe = match inline_rpe {
        Some(value) => Some(value.parse::<f64>().map_err(|_| "invalid rpe".to_string())?),
        None => None,
    };
    let mut pain_rating = None;
    let mut form_rating = None;
    for token in tokens {
        if let Some(value) = token.strip_prefix('@') {
            rpe = Some(value.parse::<f64>().map_err(|_| "invalid rpe".to_string())?);
        } else if let Some(value) = token.strip_prefix("pain=") {
            pain_rating = Some(value.parse::<i64>().map_err(|_| "invalid pain".to_string())?);
        } else if let Some(value) = token.strip_prefix("form=") {
            form_rating = Some(value.parse::<i64>().map_err(|_| "invalid form".to_string())?);
        } else {
            return Err(format!("unsupported token {token}"));
        }
    }
    let set = ParsedSet {
        weight_kg: weight_part.parse::<f64>().map_err(|_| "invalid weight".to_string())?,
        reps: reps_part.parse::<i64>().map_err(|_| "invalid reps".to_string())?,
        rpe,
        pain_rating,
        form_rating,
    };
    validate_set_input(&SetInput {
        set_type: "working".to_string(),
        weight_kg: Some(set.weight_kg),
        reps: Some(set.reps),
        target_reps: None,
        rpe: set.rpe,
        rir: None,
        rest_sec: None,
        tempo: None,
        form_rating: set.form_rating,
        pain_rating: set.pain_rating,
        notes: None,
    })?;
    Ok(set)
}

fn renumber_sets(conn: &Connection, exercise_id: &str) -> AppResult<()> {
    let mut stmt = conn.prepare("SELECT id FROM set_logs WHERE exercise_log_id = ? ORDER BY set_number, created_at").map_err(db_err)?;
    let ids = stmt.query_map(params![exercise_id], |row| row.get::<_, String>(0)).map_err(db_err)?.collect::<Result<Vec<_>, _>>().map_err(db_err)?;
    for (index, id) in ids.iter().enumerate() {
        conn.execute("UPDATE set_logs SET set_number = ?, updated_at = ? WHERE id = ?", params![index as i64 + 1, now_iso(), id]).map_err(db_err)?;
    }
    Ok(())
}

fn workout_from_row(row: &Row<'_>) -> rusqlite::Result<Workout> {
    Ok(Workout {
        id: row.get("id")?,
        date: row.get("date")?,
        title: row.get("title")?,
        workout_type: row.get("type")?,
        bodyweight_kg: row.get("bodyweight_kg")?,
        duration_min: row.get("duration_min")?,
        distance_km: row.get("distance_km")?,
        speed_kmh: row.get("speed_kmh")?,
        pace_min_per_km: row.get("pace_min_per_km")?,
        elevation_gain_m: row.get("elevation_gain_m")?,
        avg_heart_rate_bpm: row.get("avg_heart_rate_bpm")?,
        max_heart_rate_bpm: row.get("max_heart_rate_bpm")?,
        calories: row.get("calories")?,
        steps: row.get("steps")?,
        perceived_energy: row.get("perceived_energy")?,
        perceived_recovery: row.get("perceived_recovery")?,
        notes: row.get("notes")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn exercise_from_row(row: &Row<'_>) -> rusqlite::Result<Exercise> {
    Ok(Exercise {
        id: row.get("id")?,
        workout_id: row.get("workout_id")?,
        exercise_name: row.get("exercise_name")?,
        category: row.get("category")?,
        muscle_group_json: row.get("muscle_group_json")?,
        equipment: row.get("equipment")?,
        notes: row.get("notes")?,
        sort_order: row.get("sort_order")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn set_from_row(row: &Row<'_>) -> rusqlite::Result<SetLog> {
    Ok(SetLog {
        id: row.get("id")?,
        exercise_log_id: row.get("exercise_log_id")?,
        set_number: row.get("set_number")?,
        set_type: row.get("set_type")?,
        weight_kg: row.get("weight_kg")?,
        reps: row.get("reps")?,
        target_reps: row.get("target_reps")?,
        rpe: row.get("rpe")?,
        rir: row.get("rir")?,
        rest_sec: row.get("rest_sec")?,
        tempo: row.get("tempo")?,
        form_rating: row.get("form_rating")?,
        pain_rating: row.get("pain_rating")?,
        notes: row.get("notes")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn catalog_exercise_from_row(row: &Row<'_>) -> rusqlite::Result<CatalogExercise> {
    Ok(CatalogExercise {
        id: row.get("id")?,
        source_id: row.get("source_id")?,
        name: row.get("name")?,
        category: row.get("category")?,
        body_part: row.get("body_part")?,
        equipment: row.get("equipment")?,
        target: row.get("target")?,
        muscle_group: row.get("muscle_group")?,
        secondary_muscles_json: row.get("secondary_muscles_json")?,
        instructions_en: row.get("instructions_en")?,
        media_id: row.get("media_id")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn exercise_from_join_row(row: &Row<'_>) -> rusqlite::Result<Exercise> {
    Ok(Exercise {
        id: row.get(0)?,
        workout_id: row.get(1)?,
        exercise_name: row.get(2)?,
        category: row.get(3)?,
        muscle_group_json: row.get(4)?,
        equipment: row.get(5)?,
        notes: row.get(6)?,
        sort_order: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn workout_from_join_row(row: &Row<'_>) -> rusqlite::Result<Workout> {
    Ok(Workout {
        id: row.get(10)?,
        date: row.get(11)?,
        title: row.get(12)?,
        workout_type: row.get(13)?,
        bodyweight_kg: row.get(14)?,
        duration_min: row.get(15)?,
        distance_km: row.get(16)?,
        speed_kmh: row.get(17)?,
        pace_min_per_km: row.get(18)?,
        elevation_gain_m: row.get(19)?,
        avg_heart_rate_bpm: row.get(20)?,
        max_heart_rate_bpm: row.get(21)?,
        calories: row.get(22)?,
        steps: row.get(23)?,
        perceived_energy: row.get(24)?,
        perceived_recovery: row.get(25)?,
        notes: row.get(26)?,
        created_at: row.get(27)?,
        updated_at: row.get(28)?,
    })
}

fn format_workout_header(workout: &Workout) -> String {
    let mut lines = vec![
        format!("Workout: {}", workout.title.as_deref().unwrap_or("Untitled")),
        format!("Date: {}", workout.date),
        format!("Type: {}", workout.workout_type),
    ];
    if let Some(cardio) = format_cardio_workout(workout) {
        lines.push(format!("Cardio: {cardio}"));
    }
    lines.join("\n")
}

fn format_workout_details(conn: &Connection, workout: &Workout) -> AppResult<String> {
    let mut lines = vec![format_workout_header(workout), format!("ID: {}", workout.id), "".to_string()];
    let exercises = exercises_for_workout(conn, &workout.id)?;
    if exercises.is_empty() {
        lines.push("No exercises logged.".to_string());
    }
    for exercise in exercises {
        lines.push(format!("{}. {} ({})", exercise.sort_order, exercise.exercise_name, exercise.id));
        for set in sets_for_exercise(conn, &exercise.id)? {
            lines.push(format!("   {}. {}", set.set_number, format_set(&set)));
        }
    }
    Ok(lines.join("\n"))
}

fn format_catalog_summary(exercise: &CatalogExercise) -> String {
    let mut parts = vec![format!("{} {}", exercise.source_id, exercise.name)];
    if let Some(category) = &exercise.category {
        parts.push(format!("category={category}"));
    }
    if let Some(equipment) = &exercise.equipment {
        parts.push(format!("equipment={equipment}"));
    }
    if let Some(target) = &exercise.target {
        parts.push(format!("target={target}"));
    }
    parts.join(" | ")
}

fn format_catalog_details(exercise: &CatalogExercise) -> String {
    let mut lines = vec![
        exercise.name.clone(),
        format!("Source ID: {}", exercise.source_id),
    ];
    if let Some(category) = &exercise.category {
        lines.push(format!("Category: {category}"));
    }
    if let Some(body_part) = &exercise.body_part {
        lines.push(format!("Body part: {body_part}"));
    }
    if let Some(equipment) = &exercise.equipment {
        lines.push(format!("Equipment: {equipment}"));
    }
    if let Some(target) = &exercise.target {
        lines.push(format!("Target: {target}"));
    }
    if let Some(muscle_group) = &exercise.muscle_group {
        lines.push(format!("Muscle group: {muscle_group}"));
    }
    if let Ok(secondary) = serde_json::from_str::<Vec<String>>(&exercise.secondary_muscles_json) {
        if !secondary.is_empty() {
            lines.push(format!("Secondary muscles: {}", secondary.join(", ")));
        }
    }
    if let Some(media_id) = &exercise.media_id {
        lines.push(format!("Media ID: {media_id}"));
    }
    if let Some(instructions) = &exercise.instructions_en {
        lines.push("Instructions:".to_string());
        lines.push(instructions.clone());
    }
    lines.join("\n")
}

fn format_set(set: &SetLog) -> String {
    let mut text = match (set.weight_kg, set.reps) {
        (Some(weight), Some(reps)) => format!("{}kg x {}", trim_float(weight), reps),
        (Some(weight), None) => format!("{}kg", trim_float(weight)),
        (None, Some(reps)) => format!("x {reps}"),
        (None, None) => set.set_type.clone(),
    };
    if set.set_type != "working" && (set.weight_kg.is_some() || set.reps.is_some()) {
        text.push_str(&format!(" type={}", set.set_type));
    }
    if let Some(target_reps) = set.target_reps {
        text.push_str(&format!(" target={target_reps}"));
    }
    if let Some(rpe) = set.rpe {
        text.push_str(&format!(" @ RPE {}", trim_float(rpe)));
    }
    if let Some(rir) = set.rir {
        text.push_str(&format!(" RIR {}", trim_float(rir)));
    }
    if let Some(rest_sec) = set.rest_sec {
        text.push_str(&format!(" rest={rest_sec}s"));
    }
    if let Some(tempo) = &set.tempo {
        text.push_str(&format!(" tempo={tempo}"));
    }
    if let Some(pain) = set.pain_rating {
        text.push_str(&format!(" pain={pain}"));
    }
    if let Some(form) = set.form_rating {
        text.push_str(&format!(" form={form}"));
    }
    if let Some(notes) = &set.notes {
        text.push_str(&format!(" notes=\"{notes}\""));
    }
    text
}

fn format_set_value(set: &Value) -> String {
    let weight = set["weight_kg"].as_f64();
    let reps = set["reps"].as_i64();
    let mut text = match (weight, reps) {
        (Some(weight), Some(reps)) => format!("{}kg x {}", trim_float(weight), reps),
        (Some(weight), None) => format!("{}kg", trim_float(weight)),
        (None, Some(reps)) => format!("x {reps}"),
        (None, None) => set["set_type"].as_str().unwrap_or("set").to_string(),
    };
    if let Some(set_type) = set["set_type"].as_str().filter(|value| *value != "working") {
        if weight.is_some() || reps.is_some() {
            text.push_str(&format!(" type={set_type}"));
        }
    }
    if let Some(target_reps) = set["target_reps"].as_i64() {
        text.push_str(&format!(" target={target_reps}"));
    }
    if let Some(rpe) = set["rpe"].as_f64() {
        text.push_str(&format!(" @ RPE {}", trim_float(rpe)));
    }
    if let Some(rir) = set["rir"].as_f64() {
        text.push_str(&format!(" RIR {}", trim_float(rir)));
    }
    if let Some(rest_sec) = set["rest_sec"].as_i64() {
        text.push_str(&format!(" rest={rest_sec}s"));
    }
    if let Some(tempo) = set["tempo"].as_str() {
        text.push_str(&format!(" tempo={tempo}"));
    }
    if let Some(pain) = set["pain_rating"].as_i64() {
        text.push_str(&format!(" pain={pain}"));
    }
    if let Some(form) = set["form_rating"].as_i64() {
        text.push_str(&format!(" form={form}"));
    }
    if let Some(notes) = set["notes"].as_str() {
        text.push_str(&format!(" notes=\"{notes}\""));
    }
    text
}

fn format_cardio_workout(workout: &Workout) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(duration) = workout.duration_min {
        parts.push(format!("{duration} min"));
    }
    if let Some(distance) = workout.distance_km {
        parts.push(format!("{} km", trim_float(distance)));
    }
    if let Some(speed) = workout.speed_kmh {
        parts.push(format!("{} km/h", trim_float(speed)));
    }
    if let Some(pace) = workout.pace_min_per_km {
        parts.push(format!("{} min/km", trim_float(pace)));
    }
    if let Some(elevation) = workout.elevation_gain_m {
        parts.push(format!("{} m elevation", trim_float(elevation)));
    }
    if let Some(avg_hr) = workout.avg_heart_rate_bpm {
        parts.push(format!("avg HR {avg_hr} bpm"));
    }
    if let Some(max_hr) = workout.max_heart_rate_bpm {
        parts.push(format!("max HR {max_hr} bpm"));
    }
    if let Some(calories) = workout.calories {
        parts.push(format!("{calories} kcal"));
    }
    if let Some(steps) = workout.steps {
        parts.push(format!("{steps} steps"));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(", "))
    }
}

fn format_cardio_value(workout: &Value) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(duration) = workout["duration_min"].as_i64() {
        parts.push(format!("{duration} min"));
    }
    if let Some(distance) = workout["distance_km"].as_f64() {
        parts.push(format!("{} km", trim_float(distance)));
    }
    if let Some(speed) = workout["speed_kmh"].as_f64() {
        parts.push(format!("{} km/h", trim_float(speed)));
    }
    if let Some(pace) = workout["pace_min_per_km"].as_f64() {
        parts.push(format!("{} min/km", trim_float(pace)));
    }
    if let Some(elevation) = workout["elevation_gain_m"].as_f64() {
        parts.push(format!("{} m elevation", trim_float(elevation)));
    }
    if let Some(avg_hr) = workout["avg_heart_rate_bpm"].as_i64() {
        parts.push(format!("avg HR {avg_hr} bpm"));
    }
    if let Some(max_hr) = workout["max_heart_rate_bpm"].as_i64() {
        parts.push(format!("max HR {max_hr} bpm"));
    }
    if let Some(calories) = workout["calories"].as_i64() {
        parts.push(format!("{calories} kcal"));
    }
    if let Some(steps) = workout["steps"].as_i64() {
        parts.push(format!("{steps} steps"));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(", "))
    }
}

fn set_args_to_input(args: SetArgs) -> AppResult<SetInput> {
    let input = SetInput {
        set_type: args.set_type,
        weight_kg: args.weight,
        reps: args.reps,
        target_reps: args.target_reps,
        rpe: args.rpe,
        rir: args.rir,
        rest_sec: args.rest_sec,
        tempo: args.tempo,
        form_rating: args.form,
        pain_rating: args.pain,
        notes: args.notes,
    };
    validate_set_input(&input)?;
    Ok(input)
}

fn validate_set_input(input: &SetInput) -> AppResult<()> {
    validate_set_type(&input.set_type)?;
    validate_nonnegative_f64(input.weight_kg, "weight")?;
    validate_nonnegative_i64(input.reps, "reps")?;
    validate_nonnegative_i64(input.target_reps, "target_reps")?;
    validate_range_f64(input.rpe, 1.0, 10.0, "rpe")?;
    validate_nonnegative_f64(input.rir, "rir")?;
    validate_nonnegative_i64(input.rest_sec, "rest_sec")?;
    validate_range(input.form_rating, 1, 5, "form")?;
    validate_range(input.pain_rating, 0, 5, "pain")?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_cardio_args(
    distance_km: Option<f64>,
    speed_kmh: Option<f64>,
    pace_min_per_km: Option<f64>,
    elevation_gain_m: Option<f64>,
    avg_heart_rate_bpm: Option<i64>,
    max_heart_rate_bpm: Option<i64>,
    calories: Option<i64>,
    steps: Option<i64>,
) -> AppResult<()> {
    validate_nonnegative_f64(distance_km, "distance_km")?;
    validate_nonnegative_f64(speed_kmh, "speed_kmh")?;
    validate_nonnegative_f64(pace_min_per_km, "pace_min_per_km")?;
    validate_nonnegative_f64(elevation_gain_m, "elevation_gain_m")?;
    validate_nonnegative_i64(avg_heart_rate_bpm, "avg_heart_rate_bpm")?;
    validate_nonnegative_i64(max_heart_rate_bpm, "max_heart_rate_bpm")?;
    validate_nonnegative_i64(calories, "calories")?;
    validate_nonnegative_i64(steps, "steps")?;
    if let (Some(avg), Some(max)) = (avg_heart_rate_bpm, max_heart_rate_bpm) {
        if max < avg {
            return Err("max_heart_rate_bpm must be greater than or equal to avg_heart_rate_bpm".to_string());
        }
    }
    Ok(())
}

fn validate_workout_type(value: &str) -> AppResult<()> {
    validate_enum(value, &["gym", "home", "cardio", "mixed", "mobility", "other"], "workout type")
}

fn validate_category(value: Option<&str>) -> AppResult<()> {
    if let Some(value) = value {
        validate_enum(value, &["push", "pull", "legs", "core", "cardio", "mobility", "other"], "category")?;
    }
    Ok(())
}

fn validate_set_type(value: &str) -> AppResult<()> {
    validate_enum(value, &["warmup", "working", "backoff", "drop", "failure", "other"], "set type")
}

fn validate_enum(value: &str, allowed: &[&str], name: &str) -> AppResult<()> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(format!("{name} must be one of {}", allowed.join(", ")))
    }
}

fn validate_range(value: Option<i64>, min: i64, max: i64, name: &str) -> AppResult<()> {
    if value.is_some_and(|value| value < min || value > max) {
        return Err(format!("{name} must be between {min} and {max}"));
    }
    Ok(())
}

fn validate_range_f64(value: Option<f64>, min: f64, max: f64, name: &str) -> AppResult<()> {
    if value.is_some_and(|value| value < min || value > max) {
        return Err(format!("{name} must be between {} and {}", trim_float(min), trim_float(max)));
    }
    Ok(())
}

fn validate_nonnegative_i64(value: Option<i64>, name: &str) -> AppResult<()> {
    if value.is_some_and(|value| value < 0) {
        return Err(format!("{name} must be greater than or equal to 0"));
    }
    Ok(())
}

fn validate_nonnegative_f64(value: Option<f64>, name: &str) -> AppResult<()> {
    if value.is_some_and(|value| value < 0.0) {
        return Err(format!("{name} must be greater than or equal to 0"));
    }
    Ok(())
}

fn parse_range(last: Option<&str>, from_date: Option<&str>, to_date: Option<&str>) -> AppResult<(Option<String>, Option<String>)> {
    let end_date = match to_date {
        Some(value) => NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| "to must be YYYY-MM-DD".to_string())?,
        None => Local::now().date_naive(),
    };
    let mut start = match from_date {
        Some(value) => Some(NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| "from must be YYYY-MM-DD".to_string())?),
        None => None,
    };
    if let Some(value) = last {
        let value = value.trim().to_lowercase();
        start = if let Some(days) = value.strip_suffix("days").or_else(|| value.strip_suffix("day")) {
            Some(end_date - Duration::days(days.parse::<i64>().map_err(|_| "invalid --last value".to_string())?))
        } else if let Some(weeks) = value.strip_suffix("weeks").or_else(|| value.strip_suffix("week")) {
            Some(end_date - Duration::weeks(weeks.parse::<i64>().map_err(|_| "invalid --last value".to_string())?))
        } else {
            return Err("last must look like 7days or 4weeks".to_string());
        };
    }
    Ok((start.map(|d| d.to_string()), if last.is_some() || to_date.is_some() { Some(end_date.to_string()) } else { None }))
}

fn resolve_date(value: &str) -> AppResult<String> {
    if value == "today" {
        return Ok(today());
    }
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map(|d| d.to_string()).map_err(|_| "date must be YYYY-MM-DD or today".to_string())
}

fn today() -> String {
    Local::now().date_naive().to_string()
}

fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn split_csv(value: String) -> String {
    let items = value.split(',').map(|item| Value::String(item.trim().to_string())).collect::<Vec<_>>();
    Value::Array(items).to_string()
}

fn confirm(yes: bool, prompt: &str) -> AppResult<()> {
    if yes {
        return Ok(());
    }
    print!("{prompt} [y/N] ");
    io::stdout().flush().map_err(fs_err)?;
    let mut input = String::new();
    io::stdin().read_line(&mut input).map_err(fs_err)?;
    if input.trim().eq_ignore_ascii_case("y") || input.trim().eq_ignore_ascii_case("yes") {
        Ok(())
    } else {
        Err("aborted".to_string())
    }
}

fn trim_float(value: f64) -> String {
    if (value.fract()).abs() < f64::EPSILON {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

fn db_err(error: rusqlite::Error) -> String {
    error.to_string()
}

fn fs_err(error: io::Error) -> String {
    error.to_string()
}

fn json_err(error: serde_json::Error) -> String {
    error.to_string()
}

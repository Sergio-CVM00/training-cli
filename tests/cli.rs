use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_training")
}

fn temp_home(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("training-cli-{name}-{}-{stamp}", std::process::id()))
}

fn run(home: &PathBuf, args: &[&str]) -> std::process::Output {
    Command::new(bin())
        .args(args)
        .env("TRAINING_CLI_HOME", home)
        .output()
        .unwrap()
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

fn export_json(home: &PathBuf, name: &str) -> serde_json::Value {
    let out = home.join(name);
    let output = run(home, &["export", "--format", "json", "--out", out.to_str().unwrap()]);
    assert!(output.status.success(), "{}", stderr(&output));
    serde_json::from_str(&fs::read_to_string(out).unwrap()).unwrap()
}

fn created_workout_id(output: &std::process::Output) -> String {
    stdout(output)
        .lines()
        .find_map(|line| line.strip_prefix("Created workout "))
        .unwrap()
        .to_string()
}

fn created_exercise_id(output: &std::process::Output) -> String {
    stdout(output)
        .lines()
        .find_map(|line| line.strip_prefix("Created exercise "))
        .and_then(|rest| rest.split_once(':').map(|(id, _)| id.to_string()))
        .unwrap()
}

fn write_sample_catalog(home: &PathBuf) -> PathBuf {
    let path = home.join("exercises.json");
    fs::write(
        &path,
        r#"[
  {
    "id": "0001",
    "name": "barbell bench press",
    "category": "chest",
    "body_part": "chest",
    "equipment": "barbell",
    "target": "pectorals",
    "muscle_group": "pectorals",
    "secondary_muscles": ["triceps", "shoulders"],
    "instructions": {"en": "Lower the bar to your chest and press it back up."},
    "media_id": "EIeI8Vf",
    "image": null,
    "gif_url": null,
    "created_at": "2026-03-18T12:31:32.854798+00:00"
  },
  {
    "id": "0002",
    "name": "pull-up",
    "category": "back",
    "body_part": "back",
    "equipment": "body weight",
    "target": "lats",
    "muscle_group": "lats",
    "secondary_muscles": ["biceps"],
    "instructions": {"en": "Pull your chest toward the bar."},
    "media_id": "lBDjFxJ",
    "image": null,
    "gif_url": null,
    "created_at": "2026-03-18T12:31:32.854798+00:00"
  }
]"#,
    )
    .unwrap();
    path
}

#[test]
fn smoke_flow_logs_queries_context_and_exports() {
    let home = temp_home("smoke");

    let init = run(&home, &["init"]);
    assert!(init.status.success(), "{}", String::from_utf8_lossy(&init.stderr));
    assert!(stdout(&init).contains("Database:"));

    let log = run(&home, &["log", "Bench Press: 80x8@8, 80x7@8.5"]);
    assert!(log.status.success(), "{}", String::from_utf8_lossy(&log.stderr));
    assert!(stdout(&log).contains("Saved workout log"));

    let last = run(&home, &["last", "bench press"]);
    assert!(last.status.success(), "{}", String::from_utf8_lossy(&last.stderr));
    assert!(stdout(&last).contains("Bench Press - Last Session"));
    assert!(stdout(&last).contains("80kg x 8 @ RPE 8"));

    let history = run(&home, &["history", "Bench Press", "--last", "8weeks"]);
    assert!(history.status.success(), "{}", String::from_utf8_lossy(&history.stderr));
    assert!(stdout(&history).contains("Total working volume: 1200kg"));

    let context = run(&home, &["context", "--last", "4weeks", "--format", "markdown"]);
    assert!(context.status.success(), "{}", String::from_utf8_lossy(&context.stderr));
    assert!(stdout(&context).contains("# Training Context"));

    let export = run(&home, &["export", "--format", "json"]);
    assert!(export.status.success(), "{}", String::from_utf8_lossy(&export.stderr));
    assert!(stdout(&export).contains("training-export"));
    assert!(home.join("exports").read_dir().unwrap().next().is_some());
}

#[test]
fn help_is_a_successful_human_command() {
    let home = temp_home("help");
    let help = run(&home, &["--help"]);
    assert!(help.status.success(), "{}", stderr(&help));
    assert!(stdout(&help).contains("Usage: training"));
}

#[test]
fn set_deletion_renumbers_remaining_sets() {
    let home = temp_home("renumber");
    assert!(run(&home, &["init"]).status.success());
    assert!(run(&home, &["log", "Bench Press: 80x8@8, 75x9@8"]).status.success());

    let json = run(&home, &["export", "--format", "json", "--out", home.join("export.json").to_str().unwrap()]);
    assert!(json.status.success(), "{}", String::from_utf8_lossy(&json.stderr));
    let data: serde_json::Value = serde_json::from_str(&fs::read_to_string(home.join("export.json")).unwrap()).unwrap();
    let first_set_id = data["set_logs"][0]["id"].as_str().unwrap();

    let delete = run(&home, &["delete", "set", first_set_id, "--yes"]);
    assert!(delete.status.success(), "{}", String::from_utf8_lossy(&delete.stderr));

    assert!(run(&home, &["export", "--format", "json", "--out", home.join("export2.json").to_str().unwrap()]).status.success());
    let data: serde_json::Value = serde_json::from_str(&fs::read_to_string(home.join("export2.json")).unwrap()).unwrap();
    assert_eq!(data["set_logs"][0]["set_number"], 1);
}

#[test]
fn partial_log_warns_without_blocking_valid_sets() {
    let home = temp_home("parser");
    assert!(run(&home, &["init"]).status.success());

    let output = run(&home, &["log", "--partial", "Bad line\nBench Press: nope, 80x8@8 pain=1 form=4"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stderr(&output).contains("Warning: Invalid line"));
    assert!(stdout(&output).contains("80kg x 8 @ RPE 8 pain=1 form=4"));
}

#[test]
fn log_is_atomic_by_default_when_any_set_is_invalid() {
    let home = temp_home("atomic");
    assert!(run(&home, &["init"]).status.success());

    let output = run(&home, &["log", "Bench Press: nope, 80x8@8"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("Log was not saved"));

    let data = export_json(&home, "atomic.json");
    assert_eq!(data["workout_sessions"].as_array().unwrap().len(), 0);
    assert_eq!(data["exercise_logs"].as_array().unwrap().len(), 0);
    assert_eq!(data["set_logs"].as_array().unwrap().len(), 0);
}

#[test]
fn invalid_only_log_does_not_create_empty_workout() {
    let home = temp_home("invalid-only");
    assert!(run(&home, &["init"]).status.success());

    let output = run(&home, &["log", "Bad line\nBench Press: nope"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("Log was not saved"));

    let data = export_json(&home, "invalid-only.json");
    assert_eq!(data["workout_sessions"].as_array().unwrap().len(), 0);
}

#[test]
fn partial_invalid_only_log_does_not_create_empty_workout() {
    let home = temp_home("partial-invalid-only");
    assert!(run(&home, &["init"]).status.success());

    let output = run(&home, &["log", "--partial", "Bad line\nBench Press: nope"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("No valid sets found"));

    let data = export_json(&home, "partial-invalid-only.json");
    assert_eq!(data["workout_sessions"].as_array().unwrap().len(), 0);
}

#[test]
fn log_defaults_to_latest_same_day_workout() {
    let home = temp_home("latest-workout");
    assert!(run(&home, &["init"]).status.success());

    let first = run(&home, &["add", "workout", "--date", "today", "--title", "Morning", "--type", "gym"]);
    assert!(first.status.success(), "{}", stderr(&first));
    let first_id = created_workout_id(&first);
    std::thread::sleep(Duration::from_millis(5));
    let second = run(&home, &["add", "workout", "--date", "today", "--title", "Evening", "--type", "gym"]);
    assert!(second.status.success(), "{}", stderr(&second));
    let second_id = created_workout_id(&second);

    let log = run(&home, &["log", "Bench Press: 80x8@8"]);
    assert!(log.status.success(), "{}", stderr(&log));

    let first_show = run(&home, &["show", "workout", &first_id]);
    assert!(first_show.status.success(), "{}", stderr(&first_show));
    assert!(stdout(&first_show).contains("No exercises logged."));
    let second_show = run(&home, &["show", "workout", &second_id]);
    assert!(second_show.status.success(), "{}", stderr(&second_show));
    assert!(stdout(&second_show).contains("Bench Press"));
}

#[test]
fn log_can_target_explicit_workout() {
    let home = temp_home("explicit-workout");
    assert!(run(&home, &["init"]).status.success());

    let first = run(&home, &["add", "workout", "--date", "today", "--title", "Morning", "--type", "gym"]);
    assert!(first.status.success(), "{}", stderr(&first));
    let first_id = created_workout_id(&first);
    std::thread::sleep(Duration::from_millis(5));
    let second = run(&home, &["add", "workout", "--date", "today", "--title", "Evening", "--type", "gym"]);
    assert!(second.status.success(), "{}", stderr(&second));
    let second_id = created_workout_id(&second);

    let log = run(&home, &["log", "--workout", &first_id, "Bench Press: 80x8@8"]);
    assert!(log.status.success(), "{}", stderr(&log));

    let first_show = run(&home, &["show", "workout", &first_id]);
    assert!(first_show.status.success(), "{}", stderr(&first_show));
    assert!(stdout(&first_show).contains("Bench Press"));
    let second_show = run(&home, &["show", "workout", &second_id]);
    assert!(second_show.status.success(), "{}", stderr(&second_show));
    assert!(stdout(&second_show).contains("No exercises logged."));
}

#[test]
fn add_set_by_name_fails_when_today_has_duplicate_exercise_names() {
    let home = temp_home("ambiguous-set");
    assert!(run(&home, &["init"]).status.success());

    let first = run(&home, &["add", "workout", "--date", "today", "--title", "Morning", "--type", "gym"]);
    assert!(first.status.success(), "{}", stderr(&first));
    let first_id = created_workout_id(&first);
    let first_exercise = run(&home, &["add", "exercise", "--workout", &first_id, "--name", "Bench Press"]);
    assert!(first_exercise.status.success(), "{}", stderr(&first_exercise));

    std::thread::sleep(Duration::from_millis(5));
    let second = run(&home, &["add", "workout", "--date", "today", "--title", "Evening", "--type", "gym"]);
    assert!(second.status.success(), "{}", stderr(&second));
    let second_id = created_workout_id(&second);
    let second_exercise = run(&home, &["add", "exercise", "--workout", &second_id, "--name", "Bench Press"]);
    assert!(second_exercise.status.success(), "{}", stderr(&second_exercise));

    let output = run(&home, &["add", "set", "--exercise", "Bench Press", "--weight", "80", "--reps", "8"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("ambiguous today"));
}

#[test]
fn rich_set_fields_are_visible_in_human_and_markdown_retrieval() {
    let home = temp_home("rich-retrieval");
    assert!(run(&home, &["init"]).status.success());
    let workout = run(&home, &["add", "workout", "--date", "today", "--title", "Upper", "--type", "gym"]);
    assert!(workout.status.success(), "{}", stderr(&workout));
    let exercise = run(&home, &["add", "exercise", "--workout", "today", "--name", "Bench Press"]);
    assert!(exercise.status.success(), "{}", stderr(&exercise));
    let exercise_id = created_exercise_id(&exercise);
    let set = run(
        &home,
        &[
            "add",
            "set",
            "--exercise",
            &exercise_id,
            "--set-type",
            "backoff",
            "--weight",
            "70",
            "--reps",
            "8",
            "--target-reps",
            "10",
            "--rpe",
            "8",
            "--rir",
            "2",
            "--rest-sec",
            "150",
            "--tempo",
            "3-1-1",
            "--pain",
            "1",
            "--form",
            "4",
            "--notes",
            "paused",
        ],
    );
    assert!(set.status.success(), "{}", stderr(&set));
    let expected = "70kg x 8 type=backoff target=10 @ RPE 8 RIR 2 rest=150s tempo=3-1-1 pain=1 form=4 notes=\"paused\"";

    let show = run(&home, &["show", "workout", "today"]);
    assert!(show.status.success(), "{}", stderr(&show));
    assert!(stdout(&show).contains(expected));
    let last = run(&home, &["last", "Bench Press"]);
    assert!(last.status.success(), "{}", stderr(&last));
    assert!(stdout(&last).contains(expected));
    let history = run(&home, &["history", "Bench Press"]);
    assert!(history.status.success(), "{}", stderr(&history));
    assert!(stdout(&history).contains(expected));
    let context = run(&home, &["context", "--last", "4weeks", "--format", "markdown"]);
    assert!(context.status.success(), "{}", stderr(&context));
    assert!(stdout(&context).contains(expected));
}

#[test]
fn history_shows_suggestions_for_non_exact_exercise_names() {
    let home = temp_home("history-suggestions");
    assert!(run(&home, &["init"]).status.success());
    assert!(run(&home, &["log", "Bench Press: 80x8@8"]).status.success());

    let output = run(&home, &["history", "Bench"]);
    assert!(!output.status.success());
    let text = stdout(&output);
    assert!(text.contains("No exact match found"));
    assert!(text.contains("Bench Press"));
}

#[test]
fn history_for_exact_name_outside_range_reports_no_history_without_suggestions() {
    let home = temp_home("history-out-of-range");
    assert!(run(&home, &["init"]).status.success());
    assert!(run(&home, &["log", "Bench Press: 80x8@8"]).status.success());

    let output = run(&home, &["history", "Bench Press", "--from", "2020-01-01", "--to", "2020-01-02"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("No history found."));
    assert!(!text.contains("No exact match found"));
}

#[test]
fn log_to_unknown_workout_fails_without_creating_anything() {
    let home = temp_home("log-unknown-workout");
    assert!(run(&home, &["init"]).status.success());

    let output = run(&home, &["log", "--workout", "missing-id", "Bench Press: 80x8@8"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("Workout not found: missing-id"));

    let data = export_json(&home, "export-unknown-workout.json");
    assert_eq!(data["workout_sessions"].as_array().unwrap().len(), 0);
    assert_eq!(data["exercise_logs"].as_array().unwrap().len(), 0);
    assert_eq!(data["set_logs"].as_array().unwrap().len(), 0);
}

#[test]
fn exercise_catalog_can_be_imported_searched_and_shown() {
    let home = temp_home("catalog");
    assert!(run(&home, &["init"]).status.success());
    let catalog = write_sample_catalog(&home);

    let import = run(&home, &["exercises", "import", "--file", catalog.to_str().unwrap()]);
    assert!(import.status.success(), "{}", stderr(&import));
    assert!(stdout(&import).contains("Imported 2 catalog exercises"));

    let search = run(&home, &["exercises", "search", "bench"]);
    assert!(search.status.success(), "{}", stderr(&search));
    let search_text = stdout(&search);
    assert!(search_text.contains("0001 barbell bench press"));
    assert!(search_text.contains("equipment=barbell"));

    let show = run(&home, &["exercises", "show", "0001"]);
    assert!(show.status.success(), "{}", stderr(&show));
    let show_text = stdout(&show);
    assert!(show_text.contains("barbell bench press"));
    assert!(show_text.contains("Secondary muscles: triceps, shoulders"));
    assert!(show_text.contains("Lower the bar"));
}

#[test]
fn exercise_catalog_import_is_idempotent_and_search_treats_wildcards_literally() {
    let home = temp_home("catalog-reimport");
    assert!(run(&home, &["init"]).status.success());
    let catalog = write_sample_catalog(&home);

    assert!(run(&home, &["exercises", "import", "--file", catalog.to_str().unwrap()]).status.success());
    let reimport = run(&home, &["exercises", "import", "--file", catalog.to_str().unwrap()]);
    assert!(reimport.status.success(), "{}", stderr(&reimport));
    assert!(stdout(&reimport).contains("Imported 2 catalog exercises"));

    let data = export_json(&home, "export-catalog-reimport.json");
    assert_eq!(data["exercise_catalog"].as_array().unwrap().len(), 2);

    let wildcard = run(&home, &["exercises", "search", "%"]);
    assert!(wildcard.status.success(), "{}", stderr(&wildcard));
    assert!(stdout(&wildcard).contains("No catalog exercises found."));
}

#[test]
fn shorthand_log_enriches_new_exercises_from_catalog() {
    let home = temp_home("catalog-enrich");
    assert!(run(&home, &["init"]).status.success());
    let catalog = write_sample_catalog(&home);
    assert!(run(&home, &["exercises", "import", "--file", catalog.to_str().unwrap()]).status.success());

    let log = run(&home, &["log", "barbell bench press: 80x8@8"]);
    assert!(log.status.success(), "{}", stderr(&log));

    let data = export_json(&home, "catalog-enriched.json");
    assert_eq!(data["exercise_catalog"].as_array().unwrap().len(), 2);
    let exercise = &data["exercise_logs"][0];
    assert_eq!(exercise["exercise_name"], "barbell bench press");
    assert_eq!(exercise["category"], "push");
    assert_eq!(exercise["equipment"], "barbell");
    let muscles: serde_json::Value =
        serde_json::from_str(exercise["muscle_group_json"].as_str().unwrap()).unwrap();
    assert_eq!(muscles, serde_json::json!(["pectorals", "triceps", "shoulders"]));
}

#[test]
fn shorthand_log_by_catalog_id_uses_the_canonical_exercise_name() {
    let home = temp_home("catalog-id-enrich");
    assert!(run(&home, &["init"]).status.success());
    let catalog = write_sample_catalog(&home);
    assert!(run(&home, &["exercises", "import", "--file", catalog.to_str().unwrap()]).status.success());

    let log = run(&home, &["log", "0001: 80x8@8"]);
    assert!(log.status.success(), "{}", stderr(&log));
    let data = export_json(&home, "catalog-id-enriched.json");
    assert_eq!(data["exercise_logs"][0]["exercise_name"], "barbell bench press");
}

#[test]
fn repeated_log_command_replays_without_duplicating_sets() {
    let home = temp_home("idempotent-log");
    assert!(run(&home, &["init"]).status.success());

    let first = run(
        &home,
        &[
            "log",
            "--command-id",
            "workout-command-001",
            "Bench Press: 80x8@8",
        ],
    );
    assert!(first.status.success(), "{}", stderr(&first));
    let replay = run(
        &home,
        &[
            "log",
            "--command-id",
            "workout-command-001",
            "Bench Press: 80x8@8",
        ],
    );
    assert!(replay.status.success(), "{}", stderr(&replay));
    assert_eq!(stdout(&replay), stdout(&first));

    let data = export_json(&home, "idempotent-log.json");
    assert_eq!(data["workout_sessions"].as_array().unwrap().len(), 1);
    assert_eq!(data["exercise_logs"].as_array().unwrap().len(), 1);
    assert_eq!(data["set_logs"].as_array().unwrap().len(), 1);
}

#[test]
fn log_json_returns_a_stable_envelope_for_first_write_and_replay() {
    let home = temp_home("json-log");
    assert!(run(&home, &["init"]).status.success());
    let args = [
        "log",
        "--json",
        "--command-id",
        "json-workout-001",
        "Bench Press: 80x8@8",
    ];

    let first = run(&home, &args);
    assert!(first.status.success(), "{}", stderr(&first));
    let first_json: serde_json::Value = serde_json::from_str(&stdout(&first)).unwrap();
    assert_eq!(first_json["ok"], true);
    assert_eq!(first_json["meta"]["replayed"], false);
    assert_eq!(first_json["data"]["exercises"][0]["exercise"]["exercise_name"], "Bench Press");
    assert!(first_json["data"]["workout"]["created_at"]
        .as_str()
        .unwrap()
        .ends_with('Z'));

    let replay = run(&home, &args);
    assert!(replay.status.success(), "{}", stderr(&replay));
    let replay_json: serde_json::Value = serde_json::from_str(&stdout(&replay)).unwrap();
    assert_eq!(replay_json["ok"], true);
    assert_eq!(replay_json["meta"]["replayed"], true);
    assert_eq!(replay_json["data"], first_json["data"]);
}

#[test]
fn log_json_returns_a_structured_command_conflict() {
    let home = temp_home("json-log-conflict");
    assert!(run(&home, &["init"]).status.success());
    assert!(
        run(
            &home,
            &[
                "log",
                "--json",
                "--command-id",
                "json-workout-conflict",
                "Bench Press: 80x8@8",
            ],
        )
        .status
        .success()
    );

    let conflict = run(
        &home,
        &[
            "log",
            "--json",
            "--command-id",
            "json-workout-conflict",
            "Bench Press: 85x8@8",
        ],
    );
    assert!(!conflict.status.success());
    let value: serde_json::Value = serde_json::from_str(&stdout(&conflict)).unwrap();
    assert_eq!(value["ok"], false);
    assert_eq!(value["error"]["code"], "COMMAND_CONFLICT");
    assert_eq!(value["error"]["retryable"], false);
}

#[test]
fn log_json_wraps_command_line_validation_errors() {
    let home = temp_home("json-log-cli-error");
    let invalid = run(&home, &["log", "--json", "--unknown-option", "Bench Press: 80x8"]);
    assert!(!invalid.status.success());
    let value: serde_json::Value = serde_json::from_str(&stdout(&invalid)).unwrap();
    assert_eq!(value["ok"], false);
    assert_eq!(value["error"]["code"], "INVALID_COMMAND");
}

#[test]
fn reused_log_command_id_with_different_input_is_rejected() {
    let home = temp_home("conflicting-log");
    assert!(run(&home, &["init"]).status.success());
    assert!(
        run(
            &home,
            &[
                "log",
                "--command-id",
                "workout-command-conflict",
                "Bench Press: 80x8@8",
            ],
        )
        .status
        .success()
    );

    let conflict = run(
        &home,
        &[
            "log",
            "--command-id",
            "workout-command-conflict",
            "Bench Press: 85x8@8",
        ],
    );
    assert!(!conflict.status.success());
    assert!(stderr(&conflict).contains("already used with different input"));
    let data = export_json(&home, "conflicting-log.json");
    assert_eq!(data["set_logs"].as_array().unwrap().len(), 1);
}

#[test]
fn config_is_included_in_context() {
    let home = temp_home("config");
    assert!(run(&home, &["init"]).status.success());
    assert!(run(&home, &["config", "set", "goal", "fat loss without losing muscle"]).status.success());
    assert!(run(&home, &["config", "set", "constraint", "avoid full-range knee flexion"]).status.success());

    let context = run(&home, &["context", "--last", "4weeks", "--format", "markdown"]);
    assert!(context.status.success(), "{}", String::from_utf8_lossy(&context.stderr));
    let text = stdout(&context);
    assert!(text.contains("fat loss without losing muscle"));
    assert!(text.contains("avoid full-range knee flexion"));
}

#[test]
fn unreadable_config_fails_instead_of_silently_dropping_constraints() {
    let home = temp_home("unreadable-config");
    assert!(run(&home, &["init"]).status.success());
    fs::remove_file(home.join("config.json")).unwrap();
    fs::create_dir(home.join("config.json")).unwrap();

    let context = run(&home, &["context", "--last", "4weeks", "--format", "json"]);
    assert!(!context.status.success());
    assert!(stderr(&context).contains("config.json"));
}

#[test]
fn cardio_workout_fields_are_structured_and_exported() {
    let home = temp_home("cardio");
    assert!(run(&home, &["init"]).status.success());

    let created = run(
        &home,
        &[
            "add",
            "workout",
            "--date",
            "today",
            "--title",
            "Walking at 7 km/h",
            "--type",
            "cardio",
            "--duration-min",
            "15",
            "--speed-kmh",
            "7",
            "--notes",
            "Walking at 7 km/h for 15 min",
        ],
    );
    assert!(created.status.success(), "{}", String::from_utf8_lossy(&created.stderr));
    let text = stdout(&created);
    assert!(text.contains("Cardio: 15 min, 7 km/h"));

    assert!(
        run(
            &home,
            &[
                "export",
                "--format",
                "json",
                "--out",
                home.join("cardio.json").to_str().unwrap()
            ],
        )
        .status
        .success()
    );
    let data: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(home.join("cardio.json")).unwrap()).unwrap();
    assert_eq!(data["workout_sessions"][0]["type"], "cardio");
    assert_eq!(data["workout_sessions"][0]["duration_min"], 15);
    assert_eq!(data["workout_sessions"][0]["speed_kmh"], 7.0);

    let context = run(&home, &["context", "--last", "4weeks", "--format", "markdown"]);
    assert!(context.status.success(), "{}", String::from_utf8_lossy(&context.stderr));
    assert!(stdout(&context).contains("- Cardio: 15 min, 7 km/h"));
}

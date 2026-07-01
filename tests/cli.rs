use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

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
fn invalid_parser_lines_warn_without_blocking_valid_sets() {
    let home = temp_home("parser");
    assert!(run(&home, &["init"]).status.success());

    let output = run(&home, &["log", "Bad line\nBench Press: nope, 80x8@8 pain=1 form=4"]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(String::from_utf8_lossy(&output.stderr).contains("Warning: Invalid line"));
    assert!(stdout(&output).contains("80kg x 8 @ RPE 8 pain=1 form=4"));
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

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn help_exposes_new_noun_first_command_groups() {
    let mut cmd = Command::cargo_bin("banqline").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("bank"))
        .stdout(predicate::str::contains("account"))
        .stdout(predicate::str::contains("tx"))
        .stdout(predicate::str::contains("report"))
        .stdout(predicate::str::contains("sync"))
        .stdout(predicate::str::contains("doctor"));
}

#[test]
fn sync_help_exposes_targets() {
    let mut cmd = Command::cargo_bin("banqline").unwrap();
    cmd.args(["sync", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("all"))
        .stdout(predicate::str::contains("tx"))
        .stdout(predicate::str::contains("balances"))
        .stdout(predicate::str::contains("accounts"));
}

#[test]
fn bank_list_help_describes_country_filter() {
    let mut cmd = Command::cargo_bin("banqline").unwrap();
    cmd.args(["bank", "list", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--country"))
        .stdout(predicate::str::contains("--filter"));
}

#[test]
fn format_json_version_outputs_parseable_json() {
    let output = Command::cargo_bin("banqline")
        .unwrap()
        .args(["--format", "json", "version"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(parsed["name"], "banqline");
    assert!(parsed["version"].is_string());
}

#[test]
fn doctor_json_is_parseable_without_real_config() {
    let temp = tempfile::tempdir().unwrap();
    let cfg_path = temp.path().join("config.yaml");
    let output = Command::cargo_bin("banqline")
        .unwrap()
        .args([
            "--config",
            cfg_path.to_str().unwrap(),
            "--format",
            "json",
            "doctor",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(parsed["ok"].is_boolean());
    assert!(parsed["checks"].is_array());
}

#[test]
fn format_csv_doctor_outputs_csv_header() {
    let temp = tempfile::tempdir().unwrap();
    let cfg_path = temp.path().join("config.yaml");
    let output = Command::cargo_bin("banqline")
        .unwrap()
        .args([
            "--config",
            cfg_path.to_str().unwrap(),
            "--format",
            "csv",
            "doctor",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with("CHECK,STATUS,DETAIL,NEXT STEP\n"));
}

#[test]
fn legacy_commands_are_not_registered() {
    for legacy_command in [
        "banks",
        "auth",
        "accounts",
        "balances",
        "transactions",
        "summary",
        "forecast",
        "tag",
        "alerts",
    ] {
        Command::cargo_bin("banqline")
            .unwrap()
            .arg(legacy_command)
            .assert()
            .failure()
            .stderr(predicate::str::contains("unrecognized subcommand"));
    }
}

#[test]
fn json_compatibility_alias_is_not_registered() {
    Command::cargo_bin("banqline")
        .unwrap()
        .args(["--json", "version"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unexpected argument '--json'"));
}

#[test]
fn refresh_flags_are_not_registered_on_read_commands() {
    for args in [
        vec!["tx", "list", "--refresh"],
        vec!["balance", "list", "--refresh"],
        vec!["account", "list", "--refresh"],
        vec!["report", "forecast", "--refresh"],
        vec!["alert", "check", "--refresh"],
    ] {
        Command::cargo_bin("banqline")
            .unwrap()
            .args(args)
            .assert()
            .failure()
            .stderr(predicate::str::contains("unexpected argument '--refresh'"));
    }
}

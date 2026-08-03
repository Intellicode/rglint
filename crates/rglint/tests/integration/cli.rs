use std::fs;

use assert_cmd::Command;
use serde_json::Value;
use tempfile::tempdir;

fn command() -> Command {
    Command::cargo_bin("rglint").expect("rglint binary")
}

fn write_config(dir: &std::path::Path, rules: &str) {
    fs::write(
        dir.join(".rglintrc.toml"),
        format!("documents = \"query.graphql\"\n\n[rules]\n{rules}"),
    )
    .expect("config");
}

#[test]
fn json_diagnostic_is_stdout_and_fails_with_exit_one() {
    let dir = tempdir().expect("tempdir");
    fs::write(dir.path().join("query.graphql"), "query { hero }\n").expect("document");
    write_config(dir.path(), "no-anonymous-operations = \"error\"\n");

    let output = command()
        .current_dir(dir.path())
        .args(["--format", "json"])
        .output()
        .expect("run rglint");

    assert_eq!(output.status.code(), Some(1));
    assert!(
        output.stderr.is_empty(),
        "unexpected stderr: {:?}",
        output.stderr
    );
    let json: Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(json[0]["messages"][0]["ruleId"], "no-anonymous-operations");
    assert_eq!(json[0]["errorCount"], 1);
}

#[test]
fn no_config_uses_positional_documents_and_rule_override() {
    let dir = tempdir().expect("tempdir");
    fs::write(dir.path().join("query.graphql"), "query { hero }\n").expect("document");

    command()
        .current_dir(dir.path())
        .args([
            "--format",
            "json",
            "--rule",
            "no-anonymous-operations",
            "--max-warnings",
            "0",
            "query.graphql",
        ])
        .assert()
        .code(1)
        .stdout(predicates::str::contains("no-anonymous-operations"));
}

#[test]
fn max_warnings_turns_a_warning_into_exit_one() {
    let dir = tempdir().expect("tempdir");
    fs::write(dir.path().join("query.graphql"), "query { hero }\n").expect("document");
    write_config(dir.path(), "no-anonymous-operations = \"warn\"\n");

    command()
        .current_dir(dir.path())
        .args(["--format", "json", "--max-warnings", "0"])
        .assert()
        .code(1);
}

#[test]
fn bad_config_uses_exit_two_and_stderr() {
    let dir = tempdir().expect("tempdir");
    fs::write(dir.path().join("bad.toml"), "format = \"not-a-format\"\n").expect("config");

    let output = command()
        .current_dir(dir.path())
        .args(["--config", "bad.toml"])
        .output()
        .expect("run rglint");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("failed to parse config"));
}

#[test]
fn init_creates_a_starter_config_and_does_not_overwrite() {
    let dir = tempdir().expect("tempdir");
    command()
        .current_dir(dir.path())
        .arg("--init")
        .assert()
        .success();
    let path = dir.path().join(".rglintrc.toml");
    let original = fs::read_to_string(&path).expect("starter config");
    assert!(original.contains("spec-063"));

    command()
        .current_dir(dir.path())
        .arg("--init")
        .assert()
        .code(2)
        .stderr(predicates::str::contains("refusing to overwrite"));
    assert_eq!(fs::read_to_string(path).expect("config remains"), original);
}

#[test]
fn fix_dry_run_prints_a_diff_without_writing() {
    let dir = tempdir().expect("tempdir");
    let source = "query Example {\n  zz\n  aa\n}\n";
    fs::write(dir.path().join("query.graphql"), source).expect("document");
    write_config(
        dir.path(),
        "alphabetize = [\"warn\", { selections = [\"OperationDefinition\"] }]\n",
    );

    let output = command()
        .current_dir(dir.path())
        .arg("--fix-dry-run")
        .output()
        .expect("run rglint");

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--- "), "expected unified diff: {stdout}");
    assert_eq!(
        fs::read_to_string(dir.path().join("query.graphql")).unwrap(),
        source
    );
}

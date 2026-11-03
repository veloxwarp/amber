use assert_cmd::prelude::*;
use std::path::Path;
use std::process::{Command, Output};

const AMBER_YAML: &str = "assets/amber-masking.yaml";
const SECRET_KEY: &str = "ac2af4852f3de2dc6feb19b718d1cbf6c64c1ef618dafaf2b0a89cadcde240ac";

#[derive(serde::Deserialize, PartialEq, Eq, Debug, Clone)]
struct Pair {
    key: String,
    value: String,
}

fn temp_amber_yaml() -> tempfile::TempPath {
    let path = tempfile::NamedTempFile::new().unwrap().into_temp_path();
    std::fs::copy(AMBER_YAML, &path).unwrap();
    path
}

fn run_mv(path: impl AsRef<Path>, source: &str, destination: &str) -> Output {
    Command::cargo_bin("amber")
        .unwrap()
        .args(["mv", source, destination])
        .env("AMBER_YAML", path.as_ref())
        .env_remove("AMBER_SECRET")
        .output()
        .unwrap()
}

fn get_vars(path: impl AsRef<Path>) -> Vec<Pair> {
    let output = Command::cargo_bin("amber")
        .unwrap()
        .args(["print", "--style", "json"])
        .env("AMBER_YAML", path.as_ref())
        .env("AMBER_SECRET", SECRET_KEY)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn ciphertext_for(yaml: &str, name: &str) -> String {
    let value: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
    value["secrets"]
        .as_sequence()
        .unwrap()
        .iter()
        .find(|secret| secret["name"].as_str() == Some(name))
        .and_then(|secret| secret["cipher"].as_str())
        .unwrap()
        .to_owned()
}

#[test]
fn mv_renames_without_secret_key_and_preserves_value() {
    let temp = temp_amber_yaml();
    let before = get_vars(&temp);
    let before_yaml = std::fs::read_to_string(&temp).unwrap();
    let old_ciphertext = ciphertext_for(&before_yaml, "FOO");

    let output = run_mv(&temp, "FOO", "RENAMED");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let after_yaml = std::fs::read_to_string(&temp).unwrap();
    assert_eq!(ciphertext_for(&after_yaml, "RENAMED"), old_ciphertext);
    assert!(!after_yaml.lines().any(|line| line.trim() == "name: FOO"));

    let mut expected = before;
    for pair in &mut expected {
        if pair.key == "FOO" {
            pair.key = "RENAMED".to_owned();
        }
    }
    expected.sort_by(|a, b| a.key.cmp(&b.key));
    assert_eq!(get_vars(&temp), expected);
}

#[test]
fn mv_refuses_an_existing_destination() {
    let temp = temp_amber_yaml();
    let original = std::fs::read(&temp).unwrap();

    let output = run_mv(&temp, "BAR", "FOO");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Secret already exists"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(std::fs::read(&temp).unwrap(), original);
}

#[test]
fn mv_refuses_a_missing_source() {
    let temp = temp_amber_yaml();
    let original = std::fs::read(&temp).unwrap();

    let output = run_mv(&temp, "MISSING", "NEW");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Key does not exist"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(std::fs::read(&temp).unwrap(), original);
}

#[test]
fn mv_refuses_identical_names() {
    let temp = temp_amber_yaml();
    let original = std::fs::read(&temp).unwrap();

    let output = run_mv(&temp, "FOO", "FOO");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("must differ"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(std::fs::read(&temp).unwrap(), original);
}

#[test]
fn mv_validates_both_names() {
    let temp = temp_amber_yaml();
    let original = std::fs::read(&temp).unwrap();

    let output = run_mv(&temp, "foo", "NEW");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("upper case ASCII"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output = run_mv(&temp, "FOO", "new");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("upper case ASCII"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(std::fs::read(&temp).unwrap(), original);
}

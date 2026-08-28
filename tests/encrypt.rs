use assert_cmd::prelude::*;
use std::io::Write;
use std::process::Command;
use std::{path::Path, process::Stdio};

const AMBER_YAML: &str = "assets/amber-encrypt.yaml";
const SECRET_KEY: &str = "2a0fb64171010cd4584e2b658fc0a5effca4cd9ada2b2eea0262356852c60872";

fn temp_amber_yaml() -> tempfile::TempPath {
    let path = tempfile::NamedTempFile::new().unwrap().into_temp_path();
    std::fs::copy(AMBER_YAML, &path).unwrap();
    path
}

#[derive(serde::Deserialize, PartialEq, Eq, Debug)]
struct Pair {
    key: String,
    value: String,
}

fn get_vars(path: impl AsRef<Path>) -> Vec<Pair> {
    let output = Command::cargo_bin("amber")
        .unwrap()
        .arg("print")
        .arg("--style")
        .arg("json")
        .env("AMBER_YAML", path.as_ref())
        .env("AMBER_SECRET", SECRET_KEY)
        .output()
        .unwrap();
    if !output.status.success() {
        eprintln!("{}", std::str::from_utf8(&output.stderr).unwrap());
        panic!("Did not print successfully");
    }
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn empty_file() {
    let temp = temp_amber_yaml();
    assert_eq!(get_vars(&temp), vec![]);
}

#[test]
fn encrypt_cli() {
    let temp = temp_amber_yaml();
    let status = Command::cargo_bin("amber")
        .unwrap()
        .arg("encrypt")
        .arg("FOO")
        .arg("foovalue")
        .env("AMBER_YAML", temp.as_os_str())
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(
        get_vars(&temp),
        vec![Pair {
            key: "FOO".to_owned(),
            value: "foovalue".to_owned(),
        }]
    );
}

#[test]
fn encrypt_stdin() {
    let temp = temp_amber_yaml();
    let mut child = Command::cargo_bin("amber")
        .unwrap()
        .arg("encrypt")
        .arg("FOO")
        .env("AMBER_YAML", temp.as_os_str())
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    write!(&mut stdin, "foovalue via stdin").unwrap();
    std::mem::drop(stdin);
    let status = child.wait().unwrap();
    assert!(status.success());
    assert_eq!(
        get_vars(&temp),
        vec![Pair {
            key: "FOO".to_owned(),
            value: "foovalue via stdin".to_owned(),
        }]
    );
}

#[test]
fn plaintext_digests_can_be_disabled() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("amber.yaml");
    let status = Command::cargo_bin("amber")
        .unwrap()
        .args(["init", "--no-plaintext-digests", "--only-secret-key"])
        .env("AMBER_YAML", &path)
        .status()
        .unwrap();
    assert!(status.success());

    let status = Command::cargo_bin("amber")
        .unwrap()
        .args(["encrypt", "LOW_ENTROPY", "password123"])
        .env("AMBER_YAML", &path)
        .status()
        .unwrap();
    assert!(status.success());

    let yaml = std::fs::read_to_string(path).unwrap();
    assert!(yaml.contains("store_plaintext_sha256: false"));
    assert!(!yaml
        .lines()
        .any(|line| line.trim_start().starts_with("sha256:")));
}

#[cfg(unix)]
#[test]
fn write_file_uses_owner_only_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("secret");
    let status = Command::cargo_bin("amber")
        .unwrap()
        .args(["write-file", "--key", "FOO", "--dest"])
        .arg(&path)
        .env("AMBER_YAML", "assets/amber-masking.yaml")
        .env(
            "AMBER_SECRET",
            "ac2af4852f3de2dc6feb19b718d1cbf6c64c1ef618dafaf2b0a89cadcde240ac",
        )
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

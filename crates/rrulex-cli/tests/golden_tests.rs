use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;
use similar::{ChangeTag, TextDiff};

#[derive(Debug, Deserialize)]
struct FixtureCase {
    args: Vec<String>,
    #[serde(default)]
    expected_exit: i32,
    golden: Option<String>,
    stderr_contains: Option<String>,
}

fn project_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn fixture_dir() -> PathBuf {
    project_root().join("fixtures/cases")
}

fn golden_dir() -> PathBuf {
    project_root().join("golden/cases")
}

fn update_golden() -> bool {
    std::env::var("UPDATE_GOLDEN").is_ok()
}

fn normalize_newlines(input: &str) -> String {
    input.replace("\r\n", "\n")
}

fn diff_strings(expected: &str, actual: &str) -> String {
    let diff = TextDiff::from_lines(expected, actual);
    let mut out = String::new();
    for change in diff.iter_all_changes() {
        let sign = match change.tag() {
            ChangeTag::Delete => "-",
            ChangeTag::Insert => "+",
            ChangeTag::Equal => " ",
        };
        out.push_str(&format!("{sign}{change}"));
    }
    out
}

#[test]
fn fixture_cases() {
    let fixture_dir = fixture_dir();
    let golden_dir = golden_dir();

    let mut entries: Vec<_> = fs::read_dir(&fixture_dir)
        .expect("Failed to read fixtures/cases directory")
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .collect();

    entries.sort_by_key(|entry| entry.file_name());
    assert!(
        !entries.is_empty(),
        "No fixture cases found in {fixture_dir:?}"
    );

    for entry in entries {
        let case_path = entry.path();
        let case_name = case_path.file_stem().unwrap().to_string_lossy().to_string();

        let raw = fs::read_to_string(&case_path)
            .unwrap_or_else(|e| panic!("Failed to read fixture case {case_path:?}: {e}"));
        let case: FixtureCase = serde_json::from_str(&raw)
            .unwrap_or_else(|e| panic!("Invalid JSON in fixture case {case_path:?}: {e}"));

        let output = Command::new(env!("CARGO_BIN_EXE_rrulex"))
            .current_dir(project_root())
            .args(&case.args)
            .output()
            .unwrap_or_else(|e| panic!("Failed to execute rrulex for case {case_name}: {e}"));

        let status_code = output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8(output.stdout)
            .unwrap_or_else(|e| panic!("Stdout not UTF-8 for case {case_name}: {e}"));
        let stderr = String::from_utf8(output.stderr)
            .unwrap_or_else(|e| panic!("Stderr not UTF-8 for case {case_name}: {e}"));
        let stdout = normalize_newlines(&stdout);
        let stderr = normalize_newlines(&stderr);

        if status_code != case.expected_exit {
            panic!(
                "Unexpected exit code for {case_name}: got {status_code}, expected {}\n\nstdout:\n{}\n\nstderr:\n{}",
                case.expected_exit, stdout, stderr
            );
        }

        if let Some(expected_fragment) = case.stderr_contains.as_deref() {
            assert!(
                stderr.contains(expected_fragment),
                "Expected stderr for {case_name} to contain '{expected_fragment}', got:\n{stderr}"
            );
        }

        if case.expected_exit != 0 {
            continue;
        }

        let golden_name = case
            .golden
            .as_deref()
            .unwrap_or_else(|| panic!("Case {case_name} must provide a golden filename"));
        let golden_path = golden_dir.join(golden_name);

        if update_golden() {
            fs::create_dir_all(&golden_dir).expect("Failed to create golden/cases directory");
            fs::write(&golden_path, &stdout)
                .unwrap_or_else(|e| panic!("Failed to write golden file {golden_path:?}: {e}"));
            eprintln!("Updated golden file: {golden_path:?}");
            continue;
        }

        let expected = fs::read_to_string(&golden_path).unwrap_or_else(|e| {
            panic!(
                "Golden file {golden_path:?} missing for case {case_name}: {e}\n\
                 Hint: run with UPDATE_GOLDEN=1 cargo test -p rrulex --test golden_tests"
            )
        });
        let expected = normalize_newlines(&expected);

        if expected != stdout {
            let diff = diff_strings(&expected, &stdout);
            panic!(
                "Golden mismatch for {case_name} ({golden_name})\n\n{}\n\n\
                 Run with UPDATE_GOLDEN=1 to refresh snapshots",
                diff
            );
        }
    }
}

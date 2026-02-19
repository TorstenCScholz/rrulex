use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use similar::{ChangeTag, TextDiff};

fn project_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn fixture_dir() -> PathBuf {
    project_root().join("fixtures")
}

fn golden_dir() -> PathBuf {
    project_root().join("golden")
}

fn update_golden() -> bool {
    std::env::var("UPDATE_GOLDEN").is_ok()
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
fn golden_json_output() {
    let fixtures = fixture_dir();
    let golden = golden_dir();

    let mut entries: Vec<_> = fs::read_dir(&fixtures)
        .expect("Failed to read fixtures directory")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "txt"))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    assert!(
        !entries.is_empty(),
        "No fixture files found in {fixtures:?}"
    );

    for entry in entries {
        let fixture_path = entry.path();
        let stem = fixture_path.file_stem().unwrap().to_str().unwrap();
        let golden_path = golden.join(format!("{stem}.json"));

        let output = Command::new(env!("CARGO_BIN_EXE_tool-cli"))
            .arg("--format")
            .arg("json")
            .arg(&fixture_path)
            .output()
            .expect("Failed to execute tool-cli");

        assert!(
            output.status.success(),
            "tool-cli failed for {}: {}",
            stem,
            String::from_utf8_lossy(&output.stderr)
        );

        let actual = String::from_utf8(output.stdout).expect("Output is not valid UTF-8");

        if update_golden() {
            fs::create_dir_all(&golden).ok();
            fs::write(&golden_path, &actual)
                .unwrap_or_else(|e| panic!("Failed to write golden file {golden_path:?}: {e}"));
            eprintln!("Updated golden file: {golden_path:?}");
            continue;
        }

        let expected = fs::read_to_string(&golden_path).unwrap_or_else(|e| {
            panic!(
                "Golden file {golden_path:?} not found: {e}\n\
                 Hint: Run with UPDATE_GOLDEN=1 to generate golden files"
            )
        });

        if actual != expected {
            let diff = diff_strings(&expected, &actual);
            panic!(
                "Golden test mismatch for {stem}:\n\n\
                 {diff}\n\n\
                 Run with UPDATE_GOLDEN=1 to refresh snapshots"
            );
        }
    }
}

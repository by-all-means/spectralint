use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::symlink;

fn cmd() -> Command {
    #[allow(deprecated)]
    Command::cargo_bin("spectralint").unwrap()
}

fn json_output(args: &[&str]) -> serde_json::Value {
    let output = cmd().args(args).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    serde_json::from_str(&stdout).unwrap()
}

#[test]
fn clean_exits_0() {
    cmd()
        .args(["check", "tests/fixtures/clean"])
        .assert()
        .success();
}

#[test]
fn dead_refs_exits_1() {
    cmd()
        .args(["check", "tests/fixtures/dead_refs"])
        .assert()
        .failure()
        .code(1);
}

#[test]
fn vague_directives_exits_0() {
    cmd()
        .args(["check", "tests/fixtures/vague_directives"])
        .assert()
        .success();
}

#[test]
fn json_output_is_valid() {
    let output = cmd()
        .args(["check", "tests/fixtures/dead_refs", "--format", "json"])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    // dead_refs fixture has 2 dead references (scout.md, followup_drafter.md)
    assert_eq!(parsed["summary"]["errors"].as_u64().unwrap(), 2);
    assert_eq!(parsed["summary"]["warnings"].as_u64().unwrap(), 0);

    let diagnostics = parsed["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 2);
    for d in diagnostics {
        assert_eq!(d["severity"].as_str().unwrap(), "error");
        assert_eq!(d["category"].as_str().unwrap(), "dead-reference");
        assert!(d["file"].as_str().unwrap().ends_with("CLAUDE.md"));
        assert!(d["line"].as_u64().unwrap() > 0);
    }
}

#[test]
fn init_creates_config() {
    let dir = tempfile::tempdir().unwrap();
    cmd()
        .args(["init"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Created .spectralintrc.toml"));

    assert!(dir.path().join(".spectralintrc.toml").exists());
}

#[test]
fn init_fails_if_exists() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".spectralintrc.toml"), "").unwrap();
    cmd()
        .args(["init"])
        .current_dir(dir.path())
        .assert()
        .failure()
        .code(1);
}

#[test]
fn naming_drift_detected() {
    cmd()
        .args(["check", "tests/fixtures/naming_drift", "--format", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("naming-inconsistency"));
}

#[test]
fn enum_drift_detected() {
    cmd()
        .args(["check", "tests/fixtures/enum_drift", "--format", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("enum-drift"));
}

#[test]
fn inline_ignore_suppresses() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("CLAUDE.md"),
        "# Test\n\n<!-- spectralint-disable-next-line vague-directive -->\nTry to be helpful when possible.\n",
    )
    .unwrap();

    let output = cmd()
        .args([
            "check",
            &dir.path().display().to_string(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let diagnostics = parsed["diagnostics"].as_array().unwrap();

    let vague: Vec<_> = diagnostics
        .iter()
        .filter(|d| d["category"].as_str() == Some("vague-directive"))
        .collect();
    assert!(
        vague.is_empty(),
        "Expected vague directive to be suppressed"
    );
}

#[test]
fn github_output_format() {
    let output = cmd()
        .args(["check", "tests/fixtures/dead_refs", "--format", "github"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();

    // Verify complete GitHub Actions annotation format
    for line in stdout.lines() {
        assert!(
            line.starts_with("::error file="),
            "Each line should be a GitHub error annotation, got: {line}"
        );
        assert!(
            line.contains(",line="),
            "Annotation should include line number, got: {line}"
        );
        assert!(
            line.contains(",title=dead-reference::"),
            "Annotation should include category as title, got: {line}"
        );
    }
}

// ── Must-have: --fail-on tests ──────────────────────────────────────────

#[test]
fn fail_on_error_exits_0_for_warnings_only() {
    // naming_drift has warnings but no errors → --fail-on error (default) should exit 0
    cmd()
        .args(["check", "tests/fixtures/naming_drift", "--fail-on", "error"])
        .assert()
        .success();
}

#[test]
fn fail_on_warning_exits_1_for_warnings() {
    // naming_drift produces warnings → --fail-on warning should exit 1
    cmd()
        .args([
            "check",
            "tests/fixtures/naming_drift",
            "--fail-on",
            "warning",
        ])
        .assert()
        .failure()
        .code(1);
}

#[test]
fn fail_on_info_exits_1_for_info_only() {
    // vague_directives produces only info-level → --fail-on info should exit 1
    cmd()
        .args([
            "check",
            "tests/fixtures/vague_directives",
            "--fail-on",
            "info",
        ])
        .assert()
        .failure()
        .code(1);
}

#[test]
fn fail_on_warning_exits_0_for_info_only() {
    // vague_directives produces only info → --fail-on warning should exit 0
    cmd()
        .args([
            "check",
            "tests/fixtures/vague_directives",
            "--fail-on",
            "warning",
        ])
        .assert()
        .success();
}

// ── Must-have: historical file skipping ─────────────────────────────────

#[test]
fn historical_file_dead_refs_skipped() {
    // changelog.md references nonexistent file but should be skipped as historical
    let parsed = json_output(&["check", "tests/fixtures/historical", "--format", "json"]);
    let diagnostics = parsed["diagnostics"].as_array().unwrap();

    let dead_refs: Vec<_> = diagnostics
        .iter()
        .filter(|d| d["category"].as_str() == Some("dead-reference"))
        .collect();

    // CLAUDE.md references agents/scout.md which doesn't exist → should flag
    assert!(
        dead_refs.len() == 1,
        "Expected exactly 1 dead-reference (from CLAUDE.md, not changelog.md), got {}",
        dead_refs.len()
    );
    assert_eq!(dead_refs[0]["file"].as_str().unwrap(), "CLAUDE.md");
}

#[test]
fn historical_file_enum_drift_skipped() {
    // changelog.md has a table with different values, but historical files
    // should be excluded from enum drift comparison
    let parsed = json_output(&["check", "tests/fixtures/historical", "--format", "json"]);
    let diagnostics = parsed["diagnostics"].as_array().unwrap();

    let enum_drifts: Vec<_> = diagnostics
        .iter()
        .filter(|d| d["category"].as_str() == Some("enum-drift"))
        .collect();

    assert!(
        enum_drifts.is_empty(),
        "Historical files should not trigger enum drift, got {} diagnostics",
        enum_drifts.len()
    );
}

// ── Must-have: path-style historical_files ──────────────────────────────

#[test]
fn path_style_historical_pattern() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // Create config with path-style historical pattern and include all md
    fs::write(
        root.join(".spectralintrc.toml"),
        "include = [\"**/*.md\"]\nhistorical_files = [\"docs/history.md\"]\n",
    )
    .unwrap();

    // docs/history.md references nonexistent file, but should be skipped
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::write(
        root.join("docs/history.md"),
        "# History\n\nLoad `old/removed.md` for legacy.\n",
    )
    .unwrap();

    // CLAUDE.md with a clean reference
    fs::write(root.join("CLAUDE.md"), "# Instructions\n\nAll good.\n").unwrap();

    let parsed = json_output(&["check", &root.display().to_string(), "--format", "json"]);
    let diagnostics = parsed["diagnostics"].as_array().unwrap();

    let dead_refs: Vec<_> = diagnostics
        .iter()
        .filter(|d| d["category"].as_str() == Some("dead-reference"))
        .collect();

    assert!(
        dead_refs.is_empty(),
        "Path-style historical pattern should suppress dead-reference in docs/history.md"
    );
}

// ── Must-have: rule-specific suppression ────────────────────────────────

#[test]
fn suppress_dead_reference_but_not_vague_directive() {
    let parsed = json_output(&[
        "check",
        "tests/fixtures/suppress_rule_specific",
        "--format",
        "json",
    ]);
    let diagnostics = parsed["diagnostics"].as_array().unwrap();

    // dead-reference should be suppressed by the rule-specific disable
    let dead_refs: Vec<_> = diagnostics
        .iter()
        .filter(|d| d["category"].as_str() == Some("dead-reference"))
        .collect();
    assert!(
        dead_refs.is_empty(),
        "dead-reference should be suppressed by rule-specific disable"
    );

    // vague-directive should NOT be suppressed (only dead-reference was disabled)
    let vague: Vec<_> = diagnostics
        .iter()
        .filter(|d| d["category"].as_str() == Some("vague-directive"))
        .collect();
    assert!(
        !vague.is_empty(),
        "vague-directive should NOT be suppressed by dead-reference disable"
    );
}

// ── Must-have: case-only naming dedup ───────────────────────────────────

#[test]
fn case_only_naming_no_warning() {
    // "Input" vs "INPUT" should not warn (only differ by case)
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    fs::write(
        root.join("CLAUDE.md"),
        "# Config\n\n| Input | Action |\n|-------|--------|\n| a | do x |\n",
    )
    .unwrap();
    fs::write(
        root.join("AGENTS.md"),
        "# Config\n\n| INPUT | Action |\n|-------|--------|\n| a | do x |\n",
    )
    .unwrap();

    let parsed = json_output(&["check", &root.display().to_string(), "--format", "json"]);
    let diagnostics = parsed["diagnostics"].as_array().unwrap();

    let naming: Vec<_> = diagnostics
        .iter()
        .filter(|d| {
            d["category"].as_str() == Some("naming-inconsistency")
                && d["severity"].as_str() == Some("warning")
        })
        .collect();

    assert!(
        naming.is_empty(),
        "Case-only difference (Input vs INPUT) should not produce a warning, got {}",
        naming.len()
    );
}

// ── Nice-to-have: Unicode in headers/values ─────────────────────────────

#[test]
fn unicode_in_table_headers() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    fs::write(
        root.join("CLAUDE.md"),
        "# Config\n\n| Ñame | Ação |\n|------|------|\n| café | résumé |\n",
    )
    .unwrap();

    // Should not panic or crash
    let parsed = json_output(&["check", &root.display().to_string(), "--format", "json"]);
    assert!(parsed["summary"].is_object());
}

#[test]
fn unicode_in_enum_drift_values() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    fs::write(
        root.join("CLAUDE.md"),
        "# Routes\n\n| Status | Action |\n|--------|--------|\n| activo | procesar |\n| inactivo | saltar |\n",
    )
    .unwrap();
    fs::write(
        root.join("AGENTS.md"),
        "# Routes\n\n| Status | Action |\n|--------|--------|\n| activo | procesar |\n| archivado | eliminar |\n",
    )
    .unwrap();

    let parsed = json_output(&["check", &root.display().to_string(), "--format", "json"]);
    let diagnostics = parsed["diagnostics"].as_array().unwrap();

    let enum_drift: Vec<_> = diagnostics
        .iter()
        .filter(|d| d["category"].as_str() == Some("enum-drift"))
        .collect();

    assert!(
        !enum_drift.is_empty(),
        "Enum drift should detect differences in Unicode values"
    );
}

// ── Nice-to-have: empty files ───────────────────────────────────────────

#[test]
fn empty_file_does_not_crash() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    fs::write(root.join("CLAUDE.md"), "").unwrap();

    let parsed = json_output(&["check", &root.display().to_string(), "--format", "json"]);
    assert_eq!(parsed["summary"]["errors"].as_u64().unwrap(), 0);
    assert_eq!(parsed["summary"]["warnings"].as_u64().unwrap(), 0);
}

#[test]
fn whitespace_only_file() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    fs::write(root.join("CLAUDE.md"), "   \n\n  \n").unwrap();

    let parsed = json_output(&["check", &root.display().to_string(), "--format", "json"]);
    assert_eq!(parsed["summary"]["errors"].as_u64().unwrap(), 0);
}

// ── Nice-to-have: tables with missing cells ─────────────────────────────

#[test]
fn table_with_missing_cells_no_panic() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // Table where rows have fewer columns than headers
    fs::write(
        root.join("CLAUDE.md"),
        "# Routes\n\n| Status | Action | Priority |\n|--------|--------|----------|\n| active | process |\n| inactive |\n",
    )
    .unwrap();
    fs::write(
        root.join("AGENTS.md"),
        "# Routes\n\n| Status | Action | Priority |\n|--------|--------|----------|\n| active | process | high |\n",
    )
    .unwrap();

    // Should not panic
    let parsed = json_output(&["check", &root.display().to_string(), "--format", "json"]);
    assert!(parsed["summary"].is_object());
}

// ── Nice-to-have: individual vague patterns ─────────────────────────────

#[test]
fn each_builtin_vague_pattern_detected() {
    let patterns = [
        ("try to", "Try to do something."),
        ("consider", "Consider using a different approach."),
        ("if appropriate", "Run tests if appropriate."),
        ("be helpful", "Always be helpful to users."),
        ("when possible", "Cache results when possible."),
        ("when needed", "Retry when needed."),
        ("when necessary", "Escalate when necessary."),
        ("as needed", "Scale as needed."),
        ("as appropriate", "Format as appropriate."),
        ("use your judgment", "Use your judgment here."),
    ];

    for (pattern_name, line) in patterns {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        fs::write(root.join("CLAUDE.md"), format!("# Test\n\n{line}\n")).unwrap();

        let parsed = json_output(&["check", &root.display().to_string(), "--format", "json"]);
        let diagnostics = parsed["diagnostics"].as_array().unwrap();

        let vague: Vec<_> = diagnostics
            .iter()
            .filter(|d| d["category"].as_str() == Some("vague-directive"))
            .collect();

        assert!(
            !vague.is_empty(),
            "Pattern \"{pattern_name}\" should be detected in: {line}"
        );
    }
}

// ── Nice-to-have: vague patterns in code blocks NOT detected ────────────

#[test]
fn vague_pattern_in_code_block_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    fs::write(
        root.join("CLAUDE.md"),
        "# Test\n\n```\nTry to do something.\nConsider this.\n```\n",
    )
    .unwrap();

    let parsed = json_output(&["check", &root.display().to_string(), "--format", "json"]);
    let diagnostics = parsed["diagnostics"].as_array().unwrap();

    let vague: Vec<_> = diagnostics
        .iter()
        .filter(|d| d["category"].as_str() == Some("vague-directive"))
        .collect();

    assert!(
        vague.is_empty(),
        "Vague patterns in code blocks should not be flagged"
    );
}

// ── Nice-to-have: JSON summary counts ───────────────────────────────────

#[test]
fn json_summary_counts_correct() {
    let parsed = json_output(&["check", "tests/fixtures/dead_refs", "--format", "json"]);
    let summary = &parsed["summary"];
    let diagnostics = parsed["diagnostics"].as_array().unwrap();

    let errors: usize = diagnostics
        .iter()
        .filter(|d| d["severity"].as_str() == Some("error"))
        .count();
    let warnings: usize = diagnostics
        .iter()
        .filter(|d| d["severity"].as_str() == Some("warning"))
        .count();
    let infos: usize = diagnostics
        .iter()
        .filter(|d| d["severity"].as_str() == Some("info"))
        .count();

    assert_eq!(summary["errors"].as_u64().unwrap(), errors as u64);
    assert_eq!(summary["warnings"].as_u64().unwrap(), warnings as u64);
    assert_eq!(summary["info"].as_u64().unwrap(), infos as u64);
}

// ── Nice-to-have: config loading edge cases ─────────────────────────────

#[test]
fn custom_config_path() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    fs::write(root.join("CLAUDE.md"), "# Test\n\nTry to be helpful.\n").unwrap();

    // Config that disables vague-directive
    fs::write(
        root.join("custom.toml"),
        "[checkers.vague_directive]\nenabled = false\n",
    )
    .unwrap();

    let parsed = json_output(&[
        "check",
        &root.display().to_string(),
        "--format",
        "json",
        "--config",
        &root.join("custom.toml").display().to_string(),
    ]);
    let diagnostics = parsed["diagnostics"].as_array().unwrap();

    let vague: Vec<_> = diagnostics
        .iter()
        .filter(|d| d["category"].as_str() == Some("vague-directive"))
        .collect();

    assert!(
        vague.is_empty(),
        "Disabling vague_directive via config should suppress all vague-directive findings"
    );
}

#[test]
fn config_disable_dead_reference() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    fs::write(
        root.join("CLAUDE.md"),
        "# Test\n\nLoad `nonexistent.md` here.\n",
    )
    .unwrap();
    fs::write(
        root.join(".spectralintrc.toml"),
        "[checkers.dead_reference]\nenabled = false\n",
    )
    .unwrap();

    let parsed = json_output(&["check", &root.display().to_string(), "--format", "json"]);
    let diagnostics = parsed["diagnostics"].as_array().unwrap();

    let dead: Vec<_> = diagnostics
        .iter()
        .filter(|d| d["category"].as_str() == Some("dead-reference"))
        .collect();

    assert!(
        dead.is_empty(),
        "Disabling dead_reference via config should suppress all dead-reference findings"
    );
}

// ── Nice-to-have: enum drift dedup ──────────────────────────────────────

#[test]
fn enum_drift_no_duplicates() {
    let parsed = json_output(&["check", "tests/fixtures/enum_drift", "--format", "json"]);
    let diagnostics = parsed["diagnostics"].as_array().unwrap();

    let enum_drift: Vec<_> = diagnostics
        .iter()
        .filter(|d| d["category"].as_str() == Some("enum-drift"))
        .collect();

    // Check that no two diagnostics have the same (file, line, message) triple
    let mut seen = std::collections::HashSet::new();
    for d in &enum_drift {
        let key = format!(
            "{}:{}:{}",
            d["file"].as_str().unwrap(),
            d["line"].as_u64().unwrap(),
            d["message"].as_str().unwrap()
        );
        assert!(
            seen.insert(key.clone()),
            "Duplicate enum-drift diagnostic: {key}"
        );
    }
}

// ── Nice-to-have: custom pattern via config ─────────────────────────────

#[test]
fn custom_pattern_via_config() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    fs::write(
        root.join("CLAUDE.md"),
        "# Instructions\n\nTODO: implement this later.\nThis line is fine.\nFIXME: broken.\n",
    )
    .unwrap();

    fs::write(
        root.join(".spectralintrc.toml"),
        r#"
[[checkers.custom_patterns]]
name = "todo-comment"
pattern = "(?i)\\bTODO\\b"
severity = "warning"
message = "TODO comment found"

[[checkers.custom_patterns]]
name = "fixme-comment"
pattern = "(?i)\\bFIXME\\b"
severity = "error"
message = "FIXME comment found"
"#,
    )
    .unwrap();

    let parsed = json_output(&["check", &root.display().to_string(), "--format", "json"]);
    let diagnostics = parsed["diagnostics"].as_array().unwrap();

    let todo: Vec<_> = diagnostics
        .iter()
        .filter(|d| d["category"].as_str() == Some("custom:todo-comment"))
        .collect();
    let fixme: Vec<_> = diagnostics
        .iter()
        .filter(|d| d["category"].as_str() == Some("custom:fixme-comment"))
        .collect();

    assert_eq!(todo.len(), 1, "Expected 1 TODO match");
    assert_eq!(fixme.len(), 1, "Expected 1 FIXME match");
    assert_eq!(todo[0]["severity"].as_str().unwrap(), "warning");
    assert_eq!(fixme[0]["severity"].as_str().unwrap(), "error");
}

// ── Nice-to-have: extra vague patterns via config ───────────────────────

#[test]
fn extra_vague_patterns_via_config() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    fs::write(
        root.join("CLAUDE.md"),
        "# Instructions\n\nYou should maybe do this.\nThis is probably fine.\n",
    )
    .unwrap();

    fs::write(
        root.join(".spectralintrc.toml"),
        "[checkers.vague_directive]\nenabled = true\nextra_patterns = [\"(?i)\\\\bmaybe\\\\b\", \"(?i)\\\\bprobably\\\\b\"]\n",
    )
    .unwrap();

    let parsed = json_output(&["check", &root.display().to_string(), "--format", "json"]);
    let diagnostics = parsed["diagnostics"].as_array().unwrap();

    let vague: Vec<_> = diagnostics
        .iter()
        .filter(|d| d["category"].as_str() == Some("vague-directive"))
        .collect();

    let has_maybe = vague
        .iter()
        .any(|d| d["message"].as_str().unwrap().contains("maybe"));
    let has_probably = vague
        .iter()
        .any(|d| d["message"].as_str().unwrap().contains("probably"));

    assert!(has_maybe, "Expected 'maybe' to be detected as vague");
    assert!(has_probably, "Expected 'probably' to be detected as vague");
}

// ── Nice-to-have: block-level suppression ───────────────────────────────

#[test]
fn block_suppression_all_rules() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    fs::write(
        root.join("CLAUDE.md"),
        "# Test\n\n<!-- spectralint-disable -->\nLoad `nonexistent.md` here.\nTry to be helpful.\n<!-- spectralint-enable -->\n\nLoad `also_missing.md` here.\n",
    )
    .unwrap();

    let parsed = json_output(&["check", &root.display().to_string(), "--format", "json"]);
    let diagnostics = parsed["diagnostics"].as_array().unwrap();

    // nonexistent.md and vague directive on lines 4-5 should be suppressed
    // also_missing.md on line 8 should NOT be suppressed
    let dead_refs: Vec<_> = diagnostics
        .iter()
        .filter(|d| d["category"].as_str() == Some("dead-reference"))
        .collect();

    assert_eq!(
        dead_refs.len(),
        1,
        "Only the non-suppressed dead-reference should remain"
    );
    assert!(dead_refs[0]["message"]
        .as_str()
        .unwrap()
        .contains("also_missing.md"));
}

// ── Nice-to-have: ignore_files config ───────────────────────────────────

#[test]
fn ignore_files_skips_specific_files() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    fs::write(root.join("CLAUDE.md"), "# Good file\n\nThis is clean.\n").unwrap();
    fs::write(
        root.join("changelog.md"),
        "# Changelog\n\nLoad `nonexistent.md` here.\n",
    )
    .unwrap();

    fs::write(
        root.join(".spectralintrc.toml"),
        "include = [\"**/*.md\"]\nignore_files = [\"changelog.md\"]\n",
    )
    .unwrap();

    let parsed = json_output(&["check", &root.display().to_string(), "--format", "json"]);
    let diagnostics = parsed["diagnostics"].as_array().unwrap();

    let dead: Vec<_> = diagnostics
        .iter()
        .filter(|d| d["category"].as_str() == Some("dead-reference"))
        .collect();

    assert!(
        dead.is_empty(),
        "ignore_files should skip changelog.md entirely"
    );
}

// ── Nice-to-have: github output format details ──────────────────────────

#[test]
fn github_output_uses_notice_for_info() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    fs::write(root.join("CLAUDE.md"), "# Test\n\nTry to be helpful.\n").unwrap();

    cmd()
        .args(["check", &root.display().to_string(), "--format", "github"])
        .assert()
        .success()
        .stdout(predicate::str::contains("::notice file="));
}

// ── Nice-to-have: multiple files no false cross-contamination ───────────

#[test]
fn diagnostics_have_correct_file_paths() {
    let parsed = json_output(&["check", "tests/fixtures/enum_drift", "--format", "json"]);
    let diagnostics = parsed["diagnostics"].as_array().unwrap();

    for d in diagnostics {
        let file = d["file"].as_str().unwrap();
        assert!(
            file.ends_with(".md"),
            "Diagnostic file should be a .md file, got: {file}"
        );
        assert!(
            !file.contains(".."),
            "Diagnostic file path should not contain '..': {file}"
        );
    }
}

// ── Nice-to-have: init template is valid TOML ───────────────────────────

#[test]
fn init_template_is_parseable_toml() {
    let dir = tempfile::tempdir().unwrap();
    cmd()
        .args(["init"])
        .current_dir(dir.path())
        .assert()
        .success();

    let content = fs::read_to_string(dir.path().join(".spectralintrc.toml")).unwrap();
    let parsed: Result<toml::Value, _> = toml::from_str(&content);
    assert!(parsed.is_ok(), "Init template should be valid TOML");
}

// ── Scope boundaries ────────────────────────────────────────────────────

#[test]
fn scope_limits_enum_drift_comparison() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    fs::write(
        root.join("CLAUDE.md"),
        "# Routes\n\n| Status | Action |\n|--------|--------|\n| active | process |\n| pending | queue |\n",
    )
    .unwrap();

    fs::create_dir_all(root.join("reports")).unwrap();
    fs::write(
        root.join("reports/output.md"),
        "# Routes\n\n| Status | Action |\n|--------|--------|\n| active | process |\n| archived | delete |\n",
    )
    .unwrap();

    fs::write(
        root.join(".spectralintrc.toml"),
        "include = [\"**/*.md\"]\n\n[checkers.enum_drift]\nenabled = true\nscope = [\"CLAUDE.md\"]\n",
    )
    .unwrap();

    let parsed = json_output(&["check", &root.display().to_string(), "--format", "json"]);
    let diagnostics = parsed["diagnostics"].as_array().unwrap();

    let enum_drift: Vec<_> = diagnostics
        .iter()
        .filter(|d| d["category"].as_str() == Some("enum-drift"))
        .collect();

    assert!(
        enum_drift.is_empty(),
        "Scope should prevent cross-file enum drift comparison, got {} diagnostics",
        enum_drift.len()
    );
}

#[test]
fn scope_limits_naming_inconsistency() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    fs::write(
        root.join("CLAUDE.md"),
        "# Config\n\n| api_key | Value |\n|---------|-------|\n| x | 1 |\n",
    )
    .unwrap();

    fs::create_dir_all(root.join("reports")).unwrap();
    fs::write(
        root.join("reports/output.md"),
        "# Config\n\n| apiKey | Value |\n|--------|-------|\n| x | 1 |\n",
    )
    .unwrap();

    fs::write(
        root.join(".spectralintrc.toml"),
        "include = [\"**/*.md\"]\n\n[checkers.naming_inconsistency]\nenabled = true\nscope = [\"CLAUDE.md\"]\n",
    )
    .unwrap();

    let parsed = json_output(&["check", &root.display().to_string(), "--format", "json"]);
    let diagnostics = parsed["diagnostics"].as_array().unwrap();

    let naming: Vec<_> = diagnostics
        .iter()
        .filter(|d| d["category"].as_str() == Some("naming-inconsistency"))
        .collect();

    assert!(
        naming.is_empty(),
        "Scope should prevent cross-file naming inconsistency, got {} diagnostics",
        naming.len()
    );
}

#[test]
fn scope_limits_vague_directive() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    fs::write(root.join("CLAUDE.md"), "# Instructions\n\nThis is clean.\n").unwrap();

    fs::create_dir_all(root.join("reports")).unwrap();
    fs::write(
        root.join("reports/output.md"),
        "# Output\n\nTry to be helpful when possible.\n",
    )
    .unwrap();

    fs::write(
        root.join(".spectralintrc.toml"),
        "include = [\"**/*.md\"]\n\n[checkers.vague_directive]\nenabled = true\nscope = [\"CLAUDE.md\"]\n",
    )
    .unwrap();

    let parsed = json_output(&["check", &root.display().to_string(), "--format", "json"]);
    let diagnostics = parsed["diagnostics"].as_array().unwrap();

    let vague: Vec<_> = diagnostics
        .iter()
        .filter(|d| d["category"].as_str() == Some("vague-directive"))
        .collect();

    assert!(
        vague.is_empty(),
        "Out-of-scope vague directive should not be flagged, got {} diagnostics",
        vague.len()
    );
}

#[test]
fn empty_scope_preserves_current_behavior() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    fs::write(
        root.join("CLAUDE.md"),
        "# Routes\n\n| Status | Action |\n|--------|--------|\n| active | process |\n| pending | queue |\n",
    )
    .unwrap();
    fs::write(
        root.join("AGENTS.md"),
        "# Routes\n\n| Status | Action |\n|--------|--------|\n| active | process |\n| archived | delete |\n",
    )
    .unwrap();

    // No scope config at all
    let parsed = json_output(&["check", &root.display().to_string(), "--format", "json"]);
    let diagnostics = parsed["diagnostics"].as_array().unwrap();

    let enum_drift: Vec<_> = diagnostics
        .iter()
        .filter(|d| d["category"].as_str() == Some("enum-drift"))
        .collect();

    assert!(
        !enum_drift.is_empty(),
        "Without scope, enum drift should still compare all files"
    );
}

// ── Include filter ──────────────────────────────────────────────────

#[test]
fn default_include_skips_non_instruction_files() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // CLAUDE.md is clean
    fs::write(root.join("CLAUDE.md"), "# Instructions\n\nAll good.\n").unwrap();

    // reports/notes.md has a dead reference — but should NOT be scanned
    fs::create_dir_all(root.join("reports")).unwrap();
    fs::write(
        root.join("reports/notes.md"),
        "# Notes\n\nSee `nonexistent.md` for details.\n",
    )
    .unwrap();

    let parsed = json_output(&["check", &root.display().to_string(), "--format", "json"]);
    let diagnostics = parsed["diagnostics"].as_array().unwrap();

    let dead_refs: Vec<_> = diagnostics
        .iter()
        .filter(|d| d["category"].as_str() == Some("dead-reference"))
        .collect();

    assert!(
        dead_refs.is_empty(),
        "Default include should skip reports/notes.md, got {} dead-reference(s)",
        dead_refs.len()
    );
}

// ── Item 3: CLI error paths ──────────────────────────────────────────

#[test]
fn check_empty_directory_errors() {
    let dir = tempfile::tempdir().unwrap();
    // No markdown files at all
    cmd()
        .args(["check", &dir.path().display().to_string()])
        .assert()
        .failure();
}

#[test]
fn check_nonexistent_path_errors() {
    cmd()
        .args(["check", "/nonexistent/path/that/does/not/exist"])
        .assert()
        .failure();
}

// ── Item 11: Non-existent config path ────────────────────────────────

#[test]
fn nonexistent_config_path_errors() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("CLAUDE.md"), "# Test\n").unwrap();

    cmd()
        .args([
            "check",
            &dir.path().display().to_string(),
            "--config",
            "/nonexistent/config.toml",
        ])
        .assert()
        .failure();
}

// ── Item 12: Relative path output ────────────────────────────────────

#[test]
fn json_output_uses_relative_paths() {
    let parsed = json_output(&["check", "tests/fixtures/dead_refs", "--format", "json"]);
    let diagnostics = parsed["diagnostics"].as_array().unwrap();

    for d in diagnostics {
        let file = d["file"].as_str().unwrap();
        assert!(
            !file.starts_with('/'),
            "JSON output should use relative paths, got: {file}"
        );
    }
}

// ── Item 13: Text output truncation of info ──────────────────────────
// (Difficult to test text output directly since it goes to stdout with ANSI,
//  but we can verify JSON counts to ensure correctness of the data)

// ── Item 21: GitHub output format with custom category ───────────────

#[test]
fn github_output_custom_category_with_colon() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    fs::write(root.join("CLAUDE.md"), "# Test\n\nTODO: fix this later.\n").unwrap();
    fs::write(
        root.join(".spectralintrc.toml"),
        r#"
[[checkers.custom_patterns]]
name = "todo-comment"
pattern = "(?i)\\bTODO\\b"
severity = "warning"
message = "TODO found"
"#,
    )
    .unwrap();

    cmd()
        .args(["check", &root.display().to_string(), "--format", "github"])
        .assert()
        .stdout(predicate::str::contains("title=custom:todo-comment"));
}

// ── Item 22: Deterministic output ordering ───────────────────────────

#[test]
fn diagnostics_are_sorted_by_file_then_line() {
    let parsed = json_output(&["check", "tests/fixtures/enum_drift", "--format", "json"]);
    let diagnostics = parsed["diagnostics"].as_array().unwrap();

    for pair in diagnostics.windows(2) {
        let file_a = pair[0]["file"].as_str().unwrap();
        let file_b = pair[1]["file"].as_str().unwrap();
        let line_a = pair[0]["line"].as_u64().unwrap();
        let line_b = pair[1]["line"].as_u64().unwrap();

        assert!(
            (file_a, line_a) <= (file_b, line_b),
            "Diagnostics should be sorted by (file, line), got ({file_a}:{line_a}) before ({file_b}:{line_b})"
        );
    }
}

#[test]
fn include_all_md_restores_old_behavior() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // CLAUDE.md is clean
    fs::write(root.join("CLAUDE.md"), "# Instructions\n\nAll good.\n").unwrap();

    // reports/notes.md has a dead reference — should be scanned with include = ["**/*.md"]
    fs::create_dir_all(root.join("reports")).unwrap();
    fs::write(
        root.join("reports/notes.md"),
        "# Notes\n\nSee `nonexistent.md` for details.\n",
    )
    .unwrap();

    fs::write(
        root.join(".spectralintrc.toml"),
        "include = [\"**/*.md\"]\n",
    )
    .unwrap();

    let parsed = json_output(&["check", &root.display().to_string(), "--format", "json"]);
    let diagnostics = parsed["diagnostics"].as_array().unwrap();

    let dead_refs: Vec<_> = diagnostics
        .iter()
        .filter(|d| d["category"].as_str() == Some("dead-reference"))
        .collect();

    assert_eq!(
        dead_refs.len(),
        1,
        "include = [\"**/*.md\"] should scan reports/notes.md and find 1 dead-reference"
    );
}

// ── Remaining quality audit tests ──────────────────────────────────

#[test]
fn invalid_fail_on_value_errors() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::write(root.join("CLAUDE.md"), "# Instructions\n").unwrap();

    cmd()
        .args([
            "check",
            &root.display().to_string(),
            "--fail-on",
            "critical",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value 'critical'"));
}

#[test]
fn empty_results_json_output() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::write(
        root.join("CLAUDE.md"),
        "# Instructions\n\n## Output Format\n\nRespond in JSON format.\n\nAll good here.\n",
    )
    .unwrap();

    let parsed = json_output(&["check", &root.display().to_string(), "--format", "json"]);
    let diagnostics = parsed["diagnostics"].as_array().unwrap();
    assert!(
        diagnostics.is_empty(),
        "Clean project should produce 0 diagnostics"
    );
    assert_eq!(parsed["summary"]["errors"].as_u64().unwrap(), 0);
    assert_eq!(parsed["summary"]["warnings"].as_u64().unwrap(), 0);
    assert_eq!(parsed["summary"]["info"].as_u64().unwrap(), 0);
}

#[test]
fn empty_results_github_output() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::write(
        root.join("CLAUDE.md"),
        "# Instructions\n\n## Output Format\n\nRespond in JSON format.\n\nAll good here.\n",
    )
    .unwrap();

    let output = cmd()
        .args(["check", &root.display().to_string(), "--format", "github"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.trim().is_empty(),
        "Clean project in github format should produce no output, got: {stdout}"
    );
}

#[cfg(unix)]
#[test]
fn symlinked_instruction_file_is_scanned() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // Create the actual file in a subdirectory
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::write(
        root.join("docs/real-instructions.md"),
        "# Instructions\n\nLoad `agents/scout.md` for the scout agent.\n",
    )
    .unwrap();

    // Symlink it as CLAUDE.md in the root
    symlink(
        root.join("docs/real-instructions.md"),
        root.join("CLAUDE.md"),
    )
    .unwrap();

    let parsed = json_output(&["check", &root.display().to_string(), "--format", "json"]);
    let diagnostics = parsed["diagnostics"].as_array().unwrap();

    let dead_refs: Vec<_> = diagnostics
        .iter()
        .filter(|d| d["category"].as_str() == Some("dead-reference"))
        .collect();

    assert!(
        !dead_refs.is_empty(),
        "Symlinked CLAUDE.md should be scanned and produce dead-reference diagnostics"
    );
}

// ── Agent guidelines checker ─────────────────────────────────────────

#[test]
fn agent_guidelines_detected_via_fixture() {
    let parsed = json_output(&[
        "check",
        "tests/fixtures/agent_guidelines",
        "--format",
        "json",
    ]);
    let diagnostics = parsed["diagnostics"].as_array().unwrap();

    let ag: Vec<_> = diagnostics
        .iter()
        .filter(|d| d["category"].as_str() == Some("agent-guidelines"))
        .collect();

    // Should have at least: missing negatives, multi-responsibility,
    // 2x unconstrained delegation, missing output format
    assert!(
        ag.len() >= 4,
        "Expected at least 4 agent-guidelines diagnostics, got {}",
        ag.len()
    );

    // All should be info severity
    for d in &ag {
        assert_eq!(
            d["severity"].as_str().unwrap(),
            "info",
            "agent-guidelines should be info severity"
        );
    }

    // Check specific sub-checks are present
    let messages: Vec<&str> = ag.iter().map(|d| d["message"].as_str().unwrap()).collect();
    assert!(
        messages.iter().any(|m| m.contains("negative constraints")),
        "Should detect missing negative constraints"
    );
    assert!(
        messages.iter().any(|m| m.contains("responsibility")),
        "Should detect multi-responsibility"
    );
    assert!(
        messages
            .iter()
            .any(|m| m.contains("Unconstrained delegation")),
        "Should detect unconstrained delegation"
    );
    assert!(
        messages.iter().any(|m| m.contains("output format")),
        "Should detect missing output format"
    );
}

#[test]
fn agent_guidelines_disabled_via_config() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    fs::write(
        root.join("CLAUDE.md"),
        "# Build\n\nAlways run tests.\n\n# Testing\n\n# Deploy\n\n# Security\n\nDo whatever.\n",
    )
    .unwrap();

    fs::write(
        root.join(".spectralintrc.toml"),
        "[checkers.agent_guidelines]\nenabled = false\n",
    )
    .unwrap();

    let parsed = json_output(&["check", &root.display().to_string(), "--format", "json"]);
    let diagnostics = parsed["diagnostics"].as_array().unwrap();

    let ag: Vec<_> = diagnostics
        .iter()
        .filter(|d| d["category"].as_str() == Some("agent-guidelines"))
        .collect();

    assert!(
        ag.is_empty(),
        "Disabling agent_guidelines via config should suppress all agent-guidelines findings"
    );
}

#[test]
fn agent_guidelines_suppressed_inline() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    fs::write(
        root.join("CLAUDE.md"),
        "# Instructions\n\n<!-- spectralint-disable agent-guidelines -->\nAlways run tests.\nDo whatever you want.\n<!-- spectralint-enable agent-guidelines -->\n",
    )
    .unwrap();

    let parsed = json_output(&["check", &root.display().to_string(), "--format", "json"]);
    let diagnostics = parsed["diagnostics"].as_array().unwrap();

    let ag: Vec<_> = diagnostics
        .iter()
        .filter(|d| d["category"].as_str() == Some("agent-guidelines"))
        .collect();

    // Line-specific diagnostics (delegation on line 5) should be suppressed.
    // File-level diagnostics at line 1 are outside the disable block, so they may still appear.
    let delegation: Vec<_> = ag
        .iter()
        .filter(|d| {
            d["message"]
                .as_str()
                .unwrap()
                .contains("Unconstrained delegation")
        })
        .collect();

    assert!(
        delegation.is_empty(),
        "Inline suppression should suppress unconstrained delegation within the block"
    );
}

// ── Explain subcommand ───────────────────────────────────────────────

#[test]
fn json_output_includes_suggestion_field() {
    let parsed = json_output(&["check", "tests/fixtures/dead_refs", "--format", "json"]);
    let diagnostics = parsed["diagnostics"].as_array().unwrap();

    for d in diagnostics {
        assert_eq!(d["category"].as_str().unwrap(), "dead-reference");
        assert!(
            d["suggestion"].is_string(),
            "dead-reference diagnostics should include a suggestion field"
        );
        assert!(
            d["suggestion"]
                .as_str()
                .unwrap()
                .contains("Remove this reference"),
            "suggestion should contain actionable hint"
        );
    }
}

#[test]
fn text_output_includes_help_line() {
    let output = cmd()
        .args(["check", "tests/fixtures/dead_refs"])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("help:"),
        "Text output should include 'help:' suggestion lines, got:\n{stdout}"
    );
}

#[test]
fn json_output_omits_suggestion_when_none() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    fs::write(root.join("CLAUDE.md"), "# Test\n\nTODO: fix this.\n").unwrap();
    fs::write(
        root.join(".spectralintrc.toml"),
        "[[checkers.custom_patterns]]\nname = \"todo\"\npattern = \"(?i)\\\\bTODO\\\\b\"\nseverity = \"warning\"\nmessage = \"TODO found\"\n",
    )
    .unwrap();

    let parsed = json_output(&["check", &root.display().to_string(), "--format", "json"]);
    let diagnostics = parsed["diagnostics"].as_array().unwrap();

    let custom: Vec<_> = diagnostics
        .iter()
        .filter(|d| d["category"].as_str() == Some("custom:todo"))
        .collect();

    assert!(!custom.is_empty(), "Should have custom pattern match");
    for d in &custom {
        assert!(
            d.get("suggestion").is_none() || d["suggestion"].is_null(),
            "Custom pattern diagnostics should not have a suggestion field in JSON"
        );
    }
}

#[test]
fn explain_known_rule() {
    cmd()
        .args(["explain", "dead-reference"])
        .assert()
        .success()
        .stdout(predicate::str::contains("dead-reference"))
        .stdout(predicate::str::contains("Severity:"));
}

#[test]
fn explain_agent_guidelines() {
    cmd()
        .args(["explain", "agent-guidelines"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Missing negative constraints"))
        .stdout(predicate::str::contains("Multi-responsibility"))
        .stdout(predicate::str::contains("Unconstrained delegation"))
        .stdout(predicate::str::contains("Missing output format"));
}

#[test]
fn explain_no_args_lists_rules() {
    cmd()
        .args(["explain"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Available rules:"))
        .stdout(predicate::str::contains("dead-reference"))
        .stdout(predicate::str::contains("agent-guidelines"))
        .stdout(predicate::str::contains("spectralint explain <rule>"));
}

#[test]
fn explain_unknown_rule_fails() {
    cmd()
        .args(["explain", "nonexistent-rule"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("Unknown rule"))
        .stderr(predicate::str::contains("Available rules:"));
}

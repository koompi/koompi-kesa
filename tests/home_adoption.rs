//! A published v0.2.0 means a user has `~/.kode`. This drives the real binary
//! against one and checks what it leaves behind.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn write(path: &Path, content: &str) {
    fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
    fs::write(path, content).expect("write fixture");
}

struct LegacyInstall {
    home: tempfile::TempDir,
    project: tempfile::TempDir,
}

impl LegacyInstall {
    fn new() -> Self {
        let home = tempfile::tempdir().expect("temp home");
        let project = tempfile::tempdir().expect("temp project");
        let agent = home.path().join(".kode/agent");

        write(
            &agent.join("auth.json"),
            r#"{"anthropic":{"type":"api","key":"sk-fixture"}}"#,
        );
        write(&agent.join("settings.json"), r#"{"theme":"nord"}"#);

        let skills = home.path().join(".claude/skills");
        write(&skills.join("demo/SKILL.md"), "# demo\n");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&skills, home.path().join(".kode/skills")).expect("symlink");

        let session_dir = agent
            .join("sessions")
            .join(kesa::session::encode_cwd(project.path()));
        write(
            &session_dir.join("2026-08-01T10-00-00.jsonl"),
            &format!(
                "{}\n{}\n",
                serde_json::json!({
                    "type": "session",
                    "version": 3,
                    "id": "pre-rename-session",
                    "timestamp": "2026-08-01T10:00:00.000Z",
                    "cwd": project.path().display().to_string(),
                    "provider": "openai",
                    "modelId": "gpt-4o",
                    "thinkingLevel": "off",
                }),
                serde_json::json!({
                    "type": "user",
                    "content": [{"type": "text", "text": "hello from before the rename"}],
                }),
            ),
        );

        Self { home, project }
    }

    fn run_once(&self) -> String {
        let out = Command::new(PathBuf::from(env!("CARGO_BIN_EXE_kesa")))
            .arg("doctor")
            .current_dir(self.project.path())
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .env("HOME", self.home.path())
            .output()
            .expect("run kesa doctor");
        String::from_utf8_lossy(&out.stderr).into_owned()
    }
}

#[test]
fn first_run_adopts_the_legacy_home_and_leaves_it_working() {
    let install = LegacyInstall::new();
    let stderr = install.run_once();
    let home = install.home.path();

    assert!(
        stderr.contains("Adopted") && stderr.contains(".kode"),
        "adoption was not reported: {stderr}"
    );
    assert_eq!(
        fs::read_to_string(home.join(".kesa/agent/settings.json")).expect("adopted settings"),
        r#"{"theme":"nord"}"#
    );
    assert!(
        home.join(".kode/agent/settings.json").exists(),
        "the old install must keep working"
    );

    #[cfg(unix)]
    {
        let skills = home.join(".kesa/skills");
        assert!(
            skills
                .symlink_metadata()
                .expect("stat skills")
                .file_type()
                .is_symlink(),
            "the skills symlink was followed and copied"
        );
        assert_eq!(
            fs::read_link(&skills).expect("read link"),
            home.join(".claude/skills")
        );
    }
}

#[test]
fn a_session_recorded_before_the_rename_is_still_listed() {
    let install = LegacyInstall::new();
    install.run_once();

    let sessions = kesa::session_picker::list_sessions_for_project(
        install.project.path(),
        Some(&install.home.path().join(".kesa/agent/sessions")),
    );
    assert!(
        sessions.iter().any(|meta| meta.id == "pre-rename-session"),
        "the session picker lost the pre-rename session: {sessions:?}"
    );
}

#[test]
fn a_second_run_adopts_nothing() {
    let install = LegacyInstall::new();
    install.run_once();
    let stderr = install.run_once();
    assert!(
        !stderr.contains("Adopted"),
        "adoption is not idempotent: {stderr}"
    );
}

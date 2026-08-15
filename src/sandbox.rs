//! Filesystem sandbox for processes the bash tool spawns.
//!
//! `Cargo.toml` sets `unsafe_code = "forbid"`, so `Command::pre_exec` is not
//! available to apply landlock between fork and exec. Instead the child is
//! launched through a re-exec: `kesa __sandbox-exec ... -- <argv>` restricts
//! itself with landlock and then calls
//! [`std::os::unix::process::CommandExt::exec`], which is a safe fn. The pid
//! and process group survive `exec`, so the caller's process-group isolation
//! and its SIGPIPE trampoline both keep working.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::RwLock;

/// Hidden subcommand name for the re-exec trampoline.
pub const SANDBOX_EXEC_SUBCOMMAND: &str = "__sandbox-exec";

/// Directories a sandboxed child may read and execute from. Without these a
/// shell cannot even load its interpreter.
const SYSTEM_READ_PATHS: &[&str] = &[
    "/usr", "/bin", "/sbin", "/lib", "/lib64", "/etc", "/opt", "/proc", "/sys", "/run",
];

/// Device nodes need write as well: `/dev/null`, `/dev/tty`, `/dev/stderr`.
/// Their own permissions still apply; landlock only ever subtracts.
const SYSTEM_WRITE_PATHS: &[&str] = &["/dev", "/tmp", "/var/tmp"];

/// Unconfigured means off. `main` always calls [`configure`], so the binary
/// still fails closed; a library embedder that never opted in keeps the
/// behaviour it had before the sandbox existed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SandboxMode {
    Enforce,
    #[default]
    Off,
}

#[derive(Debug, Clone, Default)]
pub struct SandboxSettings {
    pub mode: SandboxMode,
    /// Extra directories the child may write, beyond the workspace.
    pub extra_writable: Vec<PathBuf>,
}

static SETTINGS: RwLock<Option<SandboxSettings>> = RwLock::new(None);

/// Install the process-wide sandbox settings. Called once from `main`.
pub fn configure(settings: SandboxSettings) {
    *SETTINGS.write().expect("sandbox settings lock") = Some(settings);
}

#[must_use]
pub fn settings() -> SandboxSettings {
    SETTINGS
        .read()
        .expect("sandbox settings lock")
        .clone()
        .unwrap_or_default()
}

/// What the sandbox is doing right now, and why.
///
/// A boolean cannot tell "this kernel refuses landlock" apart from "this
/// platform has no backend at all", and those deserve different words: the
/// first is a machine a user can change, the second is a property of their
/// operating system that no flag will fix. A user who believes they are
/// sandboxed grants permissions they would otherwise refuse, so every variant
/// here carries the sentence that explains it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxStatus {
    /// Every command the bash tool spawns is confined by `backend`.
    Enforcing { backend: &'static str },
    /// A backend exists for this platform and this kernel will not take it.
    KernelUnsupported {
        backend: &'static str,
        reason: String,
    },
    /// No backend is implemented for this platform. Not fixable by the user.
    NoBackend { platform: String },
    /// A backend is available and `--no-sandbox` turned it off.
    OffByRequest { backend: &'static str },
}

impl SandboxStatus {
    /// The status every platform without a backend reports. Always compiled so
    /// a Linux test can read the exact words a Mac would be shown.
    #[must_use]
    pub fn no_backend(platform: &str) -> Self {
        Self::NoBackend {
            platform: platform.to_string(),
        }
    }

    #[must_use]
    pub const fn is_enforcing(&self) -> bool {
        matches!(self, Self::Enforcing { .. })
    }

    /// True whenever commands run unconfined, whatever the cause.
    #[must_use]
    pub const fn is_degraded(&self) -> bool {
        !self.is_enforcing()
    }

    /// Stable machine name, for `doctor --format json` and for tests.
    #[must_use]
    pub const fn state(&self) -> &'static str {
        match self {
            Self::Enforcing { .. } => "enforcing",
            Self::KernelUnsupported { .. } => "kernel-unsupported",
            Self::NoBackend { .. } => "no-backend",
            Self::OffByRequest { .. } => "off-by-request",
        }
    }

    /// Under a dozen columns, for a statusline segment.
    #[must_use]
    pub const fn short_label(&self) -> &'static str {
        match self {
            Self::Enforcing { .. } => "sandbox on",
            Self::KernelUnsupported { .. } => "sandbox N/A",
            Self::NoBackend { .. } => "NO SANDBOX",
            Self::OffByRequest { .. } => "sandbox off",
        }
    }

    /// One line naming the state and its cause.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::Enforcing { backend } => {
                format!("commands are confined by {backend}")
            }
            Self::KernelUnsupported { backend, reason } => {
                format!("{backend} is not usable on this kernel: {reason}")
            }
            Self::NoBackend { platform } => format!(
                "no sandbox backend for {platform}; only Linux landlock is implemented, so commands run unconfined"
            ),
            Self::OffByRequest { backend } => {
                format!("{backend} is available and --no-sandbox turned it off")
            }
        }
    }

    /// What the user can do about it, when anything.
    #[must_use]
    pub const fn remediation(&self) -> Option<&'static str> {
        match self {
            Self::Enforcing { .. } => None,
            Self::KernelUnsupported { .. } => Some(
                "Boot a kernel that supports this backend, or accept unconfined commands with --no-sandbox",
            ),
            Self::NoBackend { .. } => Some(
                "Nothing on this platform can fix it. Run the agent on Linux, or treat every command as unconfined when approving it",
            ),
            Self::OffByRequest { .. } => Some("Drop --no-sandbox to confine commands again"),
        }
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use super::{SYSTEM_READ_PATHS, SYSTEM_WRITE_PATHS, SandboxStatus};
    use landlock::{
        ABI, Access, AccessFs, CompatLevel, Compatible, Ruleset, RulesetAttr, RulesetCreatedAttr,
        RulesetStatus, path_beneath_rules,
    };
    use std::path::{Path, PathBuf};

    /// The newest ABI whose additions are all plain filesystem rights. V6 and
    /// up add scoping and unix-socket path resolution, which would need their
    /// own allow rules before they could be handled without breaking sockets.
    const FS_ABI: ABI = ABI::V5;

    pub const BACKEND: &str = "landlock";

    pub fn status() -> SandboxStatus {
        let probe = || -> Result<(), landlock::RulesetError> {
            Ruleset::default()
                .set_compatibility(CompatLevel::HardRequirement)
                .handle_access(AccessFs::from_all(FS_ABI))?
                .create()?;
            Ok(())
        };
        match probe() {
            Ok(()) => SandboxStatus::Enforcing { backend: BACKEND },
            Err(err) => SandboxStatus::KernelUnsupported {
                backend: BACKEND,
                reason: err.to_string(),
            },
        }
    }

    pub fn restrict(
        workspace: &Path,
        extra_writable: &[PathBuf],
        extra_readable: &[PathBuf],
    ) -> Result<(), String> {
        let mut writable: Vec<PathBuf> = SYSTEM_WRITE_PATHS.iter().map(PathBuf::from).collect();
        writable.push(workspace.to_path_buf());
        writable.extend(extra_writable.iter().cloned());

        let mut readable: Vec<PathBuf> = SYSTEM_READ_PATHS.iter().map(PathBuf::from).collect();
        readable.extend(extra_readable.iter().cloned());

        let build = || -> Result<RulesetStatus, landlock::RulesetError> {
            Ok(Ruleset::default()
                .handle_access(AccessFs::from_all(FS_ABI))?
                .create()?
                .add_rules(path_beneath_rules(&readable, AccessFs::from_read(FS_ABI)))?
                .add_rules(path_beneath_rules(&writable, AccessFs::from_all(FS_ABI)))?
                .restrict_self()?
                .ruleset)
        };

        match build().map_err(|err| format!("failed to apply landlock: {err}"))? {
            RulesetStatus::FullyEnforced | RulesetStatus::PartiallyEnforced => Ok(()),
            RulesetStatus::NotEnforced => {
                Err("landlock reported the ruleset was not enforced".to_string())
            }
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod imp {
    use super::SandboxStatus;
    use std::path::{Path, PathBuf};

    pub fn status() -> SandboxStatus {
        SandboxStatus::no_backend(std::env::consts::OS)
    }

    pub fn restrict(
        _workspace: &Path,
        _extra_writable: &[PathBuf],
        _extra_readable: &[PathBuf],
    ) -> Result<(), String> {
        Err(status().describe())
    }
}

/// What this platform's backend can do, ignoring the mode the user chose.
///
/// Probed once: the answer is a property of the kernel, and the statusline
/// would otherwise make the syscall on every frame.
#[must_use]
pub fn backend_status() -> SandboxStatus {
    static PROBED: std::sync::OnceLock<SandboxStatus> = std::sync::OnceLock::new();
    PROBED.get_or_init(imp::status).clone()
}

/// What is actually in force for this process. This is the accessor the
/// statusline and the tool-approval overlay call.
#[must_use]
pub fn status() -> SandboxStatus {
    status_of(&settings(), backend_status())
}

/// A missing backend outranks the mode: no flag can conjure one, so saying
/// "off by request" on a Mac would name the wrong cause.
pub(crate) fn status_of(settings: &SandboxSettings, backend: SandboxStatus) -> SandboxStatus {
    match backend {
        SandboxStatus::Enforcing { backend } if settings.mode == SandboxMode::Off => {
            SandboxStatus::OffByRequest { backend }
        }
        other => other,
    }
}

/// Directories a sandboxed command may read beyond the workspace roots.
///
/// The file tools stop at the workspace; a shell cannot start without these.
/// So bash and `read` do not see one filesystem, and a status surface that hid
/// that would be the lie it exists to prevent.
#[must_use]
pub const fn system_readable_paths() -> &'static [&'static str] {
    SYSTEM_READ_PATHS
}

/// Directories a sandboxed command may write beyond the workspace roots.
#[must_use]
pub const fn system_writable_paths() -> &'static [&'static str] {
    SYSTEM_WRITE_PATHS
}

/// Process-wide opt-in: refuse to run at all rather than run unconfined.
static REQUIRE_BACKEND: RwLock<Option<bool>> = RwLock::new(None);

/// Install the opt-in from a settings file. Called once from `main`.
pub fn configure_require_backend(required: bool) {
    *REQUIRE_BACKEND.write().expect("require sandbox lock") = Some(required);
}

/// The opt-in a loaded settings file asks for.
#[must_use]
pub fn required_by_settings(config: &crate::config::Config) -> bool {
    config.require_sandbox.unwrap_or(false)
}

fn armed_by_settings() -> bool {
    REQUIRE_BACKEND
        .read()
        .expect("require sandbox lock")
        .unwrap_or(false)
}

fn armed_by_env() -> bool {
    crate::env::var("REQUIRE_SANDBOX")
        .is_some_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
}

#[must_use]
pub fn require_backend() -> bool {
    // either source arms it: a settings file without the key must not disarm
    // the environment variable
    armed_by_settings() || armed_by_env()
}

/// What armed the hard refusal, so a message can point at the source the user
/// actually set rather than the one that happens to be named in the string.
pub(crate) fn require_backend_source() -> Option<String> {
    source_label(armed_by_settings(), armed_by_env())
}

fn source_label(by_settings: bool, by_env: bool) -> Option<String> {
    let env_name = crate::env::name("REQUIRE_SANDBOX");
    match (by_settings, by_env) {
        (true, true) => Some(format!("settings key requireSandbox and {env_name}")),
        (true, false) => Some("settings key requireSandbox".to_string()),
        (false, true) => Some(env_name),
        (false, false) => None,
    }
}

/// The refusal a degraded sandbox earns when the user asked never to run
/// unconfined. `None` means the command may proceed.
pub(crate) fn unconfined_refusal(status: &SandboxStatus, required: bool) -> Option<String> {
    if !required || status.is_enforcing() {
        return None;
    }
    Some(format!(
        "refusing to run this command unconfined: {}. A confined sandbox is required (settings key requireSandbox, or {}=1), which trades running for not running unsandboxed; clear it to allow unconfined commands.",
        status.describe(),
        crate::env::name("REQUIRE_SANDBOX"),
    ))
}

/// Apply the sandbox to this process and replace it with `argv`. Only returns
/// on failure.
pub fn restrict_self_and_exec(
    workspace: &Path,
    extra_writable: &[PathBuf],
    argv: &[OsString],
) -> Result<std::convert::Infallible, String> {
    let Some((program, args)) = argv.split_first() else {
        return Err("no command given to sandbox".to_string());
    };

    // The same value the file tools read. This process parsed the --add-dir
    // arguments `wrap_command` put on its own command line.
    let roots = crate::config::workspace_roots();
    imp::restrict(workspace, extra_writable, roots.additional())?;

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        let err = Command::new(program).args(args).exec();
        Err(format!(
            "failed to exec {}: {err}",
            program.to_string_lossy()
        ))
    }

    #[cfg(not(unix))]
    {
        let _ = (program, args);
        Err("re-exec sandboxing requires a Unix platform".to_string())
    }
}

/// Rewrite `command` so it runs under the sandbox trampoline.
///
/// Returns the command untouched when the sandbox is off. Returns an error,
/// rather than an unsandboxed command, when the sandbox is on but unavailable.
pub fn wrap_command(command: Command, workspace: &Path) -> std::io::Result<Command> {
    wrap_command_with(
        &settings(),
        &crate::config::workspace_roots(),
        command,
        workspace,
    )
}

pub(crate) fn wrap_command_with(
    settings: &SandboxSettings,
    roots: &crate::config::WorkspaceRoots,
    command: Command,
    workspace: &Path,
) -> std::io::Result<Command> {
    // The opt-in is checked before the mode, or --no-sandbox would be a way
    // around the very refusal the user asked for.
    let status = status_of(settings, backend_status());
    if let Some(refusal) = unconfined_refusal(&status, require_backend()) {
        return Err(std::io::Error::other(refusal));
    }

    if settings.mode == SandboxMode::Off {
        return Ok(command);
    }

    if status.is_degraded() {
        return Err(std::io::Error::other(format!(
            "{}. Re-run with --no-sandbox to allow unsandboxed commands.",
            status.describe()
        )));
    }

    // The trampoline may run with a different cwd than this process, so the
    // workspace has to reach it as an absolute path.
    let workspace = workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf());
    let exe = std::env::current_exe()?;
    let mut wrapped = Command::new(exe);
    // Root flags precede the subcommand: the trampoline resolves them through
    // the same CLI parse the agent process did, so both layers see one set.
    for root in roots.additional() {
        wrapped.arg("--add-dir").arg(root);
    }
    wrapped.arg(SANDBOX_EXEC_SUBCOMMAND);
    wrapped.arg("--workspace").arg(&workspace);
    for path in &settings.extra_writable {
        wrapped.arg("--write").arg(path);
    }
    wrapped.arg("--");
    wrapped.arg(command.get_program());
    wrapped.args(command.get_args());
    Ok(wrapped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrap_command_is_a_no_op_when_the_sandbox_is_off() {
        let mut command = Command::new("/bin/echo");
        command.arg("hi");
        let settings = SandboxSettings {
            mode: SandboxMode::Off,
            extra_writable: Vec::new(),
        };
        let wrapped = wrap_command_with(
            &settings,
            &crate::config::WorkspaceRoots::for_workspace(Path::new("/tmp")),
            command,
            Path::new("/tmp"),
        )
        .expect("no-op wrap");
        assert_eq!(wrapped.get_program(), "/bin/echo");
        assert_eq!(wrapped.get_args().count(), 1);
    }

    #[test]
    fn wrap_command_puts_the_original_argv_after_the_separator() {
        let mut command = Command::new("/bin/sh");
        command.arg("-c").arg("echo hi");
        let settings = SandboxSettings {
            mode: SandboxMode::Enforce,
            extra_writable: vec![PathBuf::from("/var/cache")],
        };
        let wrapped = wrap_command_with(
            &settings,
            &crate::config::WorkspaceRoots::for_workspace(Path::new("/work")),
            command,
            Path::new("/work"),
        )
        .expect("sandbox is available here");
        let args: Vec<String> = wrapped
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            vec![
                SANDBOX_EXEC_SUBCOMMAND,
                "--workspace",
                "/work",
                "--write",
                "/var/cache",
                "--",
                "/bin/sh",
                "-c",
                "echo hi",
            ]
        );
    }

    #[test]
    fn landlock_is_available_on_this_kernel() {
        assert!(
            backend_status().is_enforcing(),
            "this machine reports landlock in /sys/kernel/security/lsm, got {:?}",
            backend_status()
        );
    }

    /// The whole point of the status type: a caller must be able to tell a
    /// kernel that refuses landlock from a platform that never had it.
    #[test]
    fn degraded_states_do_not_share_a_sentence() {
        let kernel = SandboxStatus::KernelUnsupported {
            backend: "landlock",
            reason: "the kernel does not support Landlock ABI v5".to_string(),
        };
        let platform = SandboxStatus::no_backend("macos");

        println!("kernel-unsupported: {}", kernel.describe());
        println!("no-backend:         {}", platform.describe());

        assert_ne!(kernel.describe(), platform.describe());
        assert_ne!(kernel.short_label(), platform.short_label());
        assert_ne!(kernel.state(), platform.state());
        assert!(kernel.is_degraded() && platform.is_degraded());
        assert!(
            platform.describe().contains("macos"),
            "the platform with no backend has to be named: {}",
            platform.describe()
        );
    }

    #[test]
    fn no_sandbox_is_reported_as_the_users_own_choice_not_as_a_missing_backend() {
        let off = status_of(
            &SandboxSettings {
                mode: SandboxMode::Off,
                extra_writable: Vec::new(),
            },
            SandboxStatus::Enforcing {
                backend: "landlock",
            },
        );
        assert_eq!(off.state(), "off-by-request");
        assert!(off.is_degraded());
    }

    /// `--no-sandbox` must not be able to answer a platform that has no
    /// backend, or the status would name a cause the user could undo.
    #[test]
    fn no_sandbox_cannot_turn_a_missing_backend_into_a_choice() {
        let status = status_of(
            &SandboxSettings {
                mode: SandboxMode::Off,
                extra_writable: Vec::new(),
            },
            SandboxStatus::no_backend("windows"),
        );
        assert_eq!(status.state(), "no-backend");
    }

    #[test]
    fn the_opt_in_refuses_every_degraded_state_and_permits_the_enforcing_one() {
        let enforcing = SandboxStatus::Enforcing {
            backend: "landlock",
        };
        assert_eq!(unconfined_refusal(&enforcing, true), None);
        assert_eq!(
            unconfined_refusal(&SandboxStatus::no_backend("macos"), false),
            None
        );

        for degraded in [
            SandboxStatus::no_backend("macos"),
            SandboxStatus::OffByRequest {
                backend: "landlock",
            },
            SandboxStatus::KernelUnsupported {
                backend: "landlock",
                reason: "no landlock".to_string(),
            },
        ] {
            let refusal = unconfined_refusal(&degraded, true)
                .unwrap_or_else(|| panic!("{} must be refused", degraded.state()));
            println!("{}: {refusal}", degraded.state());
            assert!(refusal.contains("refusing to run"));
            assert!(refusal.contains("KESA_REQUIRE_SANDBOX"));
        }
    }

    /// The settings key is the only way to arm the refusal without an
    /// environment variable, so the name, the default and the refusal it earns
    /// are all load-bearing.
    #[test]
    fn the_settings_key_arms_the_hard_refusal_and_its_absence_does_not() {
        let degraded = SandboxStatus::no_backend("macos");

        let armed: crate::config::Config =
            serde_json::from_str(r#"{"requireSandbox": true}"#).expect("settings parse");
        assert!(required_by_settings(&armed));
        let refusal = unconfined_refusal(&degraded, required_by_settings(&armed))
            .expect("requireSandbox must refuse a degraded sandbox");
        println!("{refusal}");
        assert!(refusal.contains("requireSandbox"));

        let snake: crate::config::Config =
            serde_json::from_str(r#"{"require_sandbox": true}"#).expect("settings parse");
        assert!(required_by_settings(&snake));

        for absent in ["{}", r#"{"requireSandbox": false}"#] {
            let config: crate::config::Config =
                serde_json::from_str(absent).expect("settings parse");
            assert!(!required_by_settings(&config), "{absent}");
            assert_eq!(
                unconfined_refusal(&degraded, required_by_settings(&config)),
                None,
                "{absent} must leave the default alone"
            );
        }
    }

    /// Telling a user to unset an environment variable they never set is advice
    /// that cannot work, so the message names whichever source armed it.
    #[test]
    fn the_refusal_message_names_the_source_that_armed_it() {
        let env_name = crate::env::name("REQUIRE_SANDBOX");

        let settings_only = source_label(true, false).expect("settings arms it");
        assert!(settings_only.contains("requireSandbox"));
        assert!(!settings_only.contains(&env_name));

        let env_only = source_label(false, true).expect("env arms it");
        assert_eq!(env_only, env_name);

        let both = source_label(true, true).expect("either arms it");
        assert!(both.contains("requireSandbox") && both.contains(&env_name));

        assert_eq!(source_label(false, false), None);
    }

    /// A cloned repository carries `.kesa/settings.json`, so a project file
    /// must not be able to switch off a refusal the machine's owner armed.
    #[test]
    fn a_project_settings_file_cannot_disarm_the_global_hard_refusal() {
        let global: crate::config::Config =
            serde_json::from_str(r#"{"requireSandbox": true}"#).expect("settings parse");
        let project: crate::config::Config =
            serde_json::from_str(r#"{"requireSandbox": false}"#).expect("settings parse");
        assert!(required_by_settings(&crate::config::Config::merge(
            global, project
        )));
    }

    /// A sandboxed shell reaches paths the file tools refuse. The status has to
    /// be able to say so; hiding it is the failure mode this module exists for.
    #[test]
    fn the_system_grants_are_readable_by_a_caller() {
        assert!(system_readable_paths().contains(&"/etc"));
        assert!(system_writable_paths().contains(&"/tmp"));
        assert!(system_writable_paths().contains(&"/var/tmp"));
    }
}

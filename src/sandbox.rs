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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability {
    Available,
    Unavailable(String),
}

impl Availability {
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::Available => "landlock available".to_string(),
            Self::Unavailable(reason) => reason.clone(),
        }
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use super::{Availability, SYSTEM_READ_PATHS, SYSTEM_WRITE_PATHS};
    use landlock::{
        ABI, Access, AccessFs, CompatLevel, Compatible, Ruleset, RulesetAttr, RulesetCreatedAttr,
        RulesetStatus, path_beneath_rules,
    };
    use std::path::{Path, PathBuf};

    /// The newest ABI whose additions are all plain filesystem rights. V6 and
    /// up add scoping and unix-socket path resolution, which would need their
    /// own allow rules before they could be handled without breaking sockets.
    const FS_ABI: ABI = ABI::V5;

    pub fn availability() -> Availability {
        let probe = || -> Result<(), landlock::RulesetError> {
            Ruleset::default()
                .set_compatibility(CompatLevel::HardRequirement)
                .handle_access(AccessFs::from_all(FS_ABI))?
                .create()?;
            Ok(())
        };
        match probe() {
            Ok(()) => Availability::Available,
            Err(err) => {
                Availability::Unavailable(format!("landlock is not usable on this kernel: {err}"))
            }
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
    use super::Availability;
    use std::path::{Path, PathBuf};

    pub fn availability() -> Availability {
        Availability::Unavailable(format!(
            "no sandbox backend for {}; only Linux landlock is implemented",
            std::env::consts::OS
        ))
    }

    pub fn restrict(
        _workspace: &Path,
        _extra_writable: &[PathBuf],
        _extra_readable: &[PathBuf],
    ) -> Result<(), String> {
        Err(availability().describe())
    }
}

#[must_use]
pub fn availability() -> Availability {
    imp::availability()
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
    if settings.mode == SandboxMode::Off {
        return Ok(command);
    }

    if let Availability::Unavailable(reason) = availability() {
        return Err(std::io::Error::other(format!(
            "{reason}. Re-run with --no-sandbox to allow unsandboxed commands."
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
        assert_eq!(
            availability(),
            Availability::Available,
            "this machine reports landlock in /sys/kernel/security/lsm"
        );
    }
}

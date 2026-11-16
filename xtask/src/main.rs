//! Repository task runner.

use std::env;
use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

mod protocol_codegen;

const MANUAL_TEST_ACKNOWLEDGEMENT: &str = "I_UNDERSTAND_TETHER_INJECTS_INPUT";

#[derive(Clone, Debug, Eq, PartialEq)]
enum Task {
    Check,
    Codegen,
    CheckGenerated,
    CodegenRust {
        descriptor: PathBuf,
        output: PathBuf,
    },
    Fixtures,
    Help,
    ManualTestGuard {
        scope: String,
    },
    PackagePreflight,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CommandSpec {
    program: &'static str,
    arguments: &'static [&'static str],
}

#[derive(Debug, Eq, PartialEq)]
enum TaskError {
    CommandFailed {
        program: String,
        status: Option<i32>,
    },
    InvalidArguments(String),
    MissingEnvironment(&'static str),
    MissingPath(PathBuf),
    NotWorkspace(PathBuf),
    Spawn {
        program: String,
        message: String,
    },
}

impl fmt::Display for TaskError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CommandFailed { program, status } => {
                write!(formatter, "{program} failed with exit status {status:?}")
            }
            Self::InvalidArguments(message) => formatter.write_str(message),
            Self::MissingEnvironment(name) => {
                write!(
                    formatter,
                    "required environment variable {name} is missing or invalid"
                )
            }
            Self::MissingPath(path) => {
                write!(formatter, "required path is missing: {}", path.display())
            }
            Self::NotWorkspace(path) => {
                write!(
                    formatter,
                    "could not find the Tether workspace above {}",
                    path.display()
                )
            }
            Self::Spawn { program, message } => {
                write!(formatter, "could not start {program}: {message}")
            }
        }
    }
}

trait Environment {
    fn value(&self, name: &str) -> Option<OsString>;
}

struct ProcessEnvironment;

impl Environment for ProcessEnvironment {
    fn value(&self, name: &str) -> Option<OsString> {
        env::var_os(name)
    }
}

trait Runner {
    fn run(&mut self, root: &Path, specification: &CommandSpec) -> Result<(), TaskError>;
}

struct ProcessRunner;

impl Runner for ProcessRunner {
    fn run(&mut self, root: &Path, specification: &CommandSpec) -> Result<(), TaskError> {
        let mut command = Command::new(specification.program);
        command
            .args(specification.arguments)
            .current_dir(root)
            .env("CARGO_TERM_COLOR", "always");
        let status = command.status().map_err(|error| TaskError::Spawn {
            program: specification.program.to_owned(),
            message: error.to_string(),
        })?;
        if status.success() {
            Ok(())
        } else {
            Err(TaskError::CommandFailed {
                program: specification.program.to_owned(),
                status: status.code(),
            })
        }
    }
}

fn parse_task(arguments: &[OsString]) -> Result<Task, TaskError> {
    let Some(command) = arguments.first() else {
        return Ok(Task::Help);
    };
    let command = command.to_str().ok_or_else(|| {
        TaskError::InvalidArguments("xtask arguments must be valid Unicode".to_owned())
    })?;

    match command {
        "check" => require_no_more_arguments(arguments, Task::Check),
        "codegen" => require_no_more_arguments(arguments, Task::Codegen),
        "check-generated" => require_no_more_arguments(arguments, Task::CheckGenerated),
        "codegen-rust" if arguments.len() == 3 => Ok(Task::CodegenRust {
            descriptor: PathBuf::from(&arguments[1]),
            output: PathBuf::from(&arguments[2]),
        }),
        "fixtures" => require_no_more_arguments(arguments, Task::Fixtures),
        "help" | "--help" | "-h" => require_no_more_arguments(arguments, Task::Help),
        "manual-test-guard" => {
            if arguments.len() != 2 {
                return Err(TaskError::InvalidArguments(
                    "usage: cargo xtask manual-test-guard <scope>".to_owned(),
                ));
            }
            let scope = arguments[1]
                .to_str()
                .filter(|scope| !scope.trim().is_empty())
                .ok_or_else(|| {
                    TaskError::InvalidArguments(
                        "manual-test-guard scope must be non-empty Unicode".to_owned(),
                    )
                })?;
            Ok(Task::ManualTestGuard {
                scope: scope.to_owned(),
            })
        }
        "package-preflight" => require_no_more_arguments(arguments, Task::PackagePreflight),
        _ => Err(TaskError::InvalidArguments(format!(
            "unknown xtask command {command:?}; run cargo xtask help"
        ))),
    }
}

fn require_no_more_arguments(arguments: &[OsString], task: Task) -> Result<Task, TaskError> {
    if arguments.len() == 1 {
        Ok(task)
    } else {
        Err(TaskError::InvalidArguments(format!(
            "command {} does not accept arguments",
            arguments[0].to_string_lossy()
        )))
    }
}

fn find_workspace_root(start: &Path) -> Result<PathBuf, TaskError> {
    let mut candidate = start.to_path_buf();
    loop {
        if candidate.join("SPEC.md").is_file()
            && candidate.join("Cargo.toml").is_file()
            && candidate.join("pnpm-workspace.yaml").is_file()
        {
            return Ok(candidate);
        }
        if !candidate.pop() {
            return Err(TaskError::NotWorkspace(start.to_path_buf()));
        }
    }
}

fn check_commands() -> &'static [CommandSpec] {
    &[
        CommandSpec {
            program: "cargo",
            arguments: &["fmt", "--all", "--check"],
        },
        CommandSpec {
            program: "cargo",
            arguments: &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--",
                "-D",
                "warnings",
            ],
        },
        CommandSpec {
            program: "cargo",
            arguments: &["test", "--workspace", "--all-targets"],
        },
        CommandSpec {
            program: "cargo",
            arguments: &["build", "--workspace", "--all-targets"],
        },
        CommandSpec {
            program: "pnpm",
            arguments: &["check:types"],
        },
        CommandSpec {
            program: "pnpm",
            arguments: &["check:protocol"],
        },
        CommandSpec {
            program: "pnpm",
            arguments: &["check:generated"],
        },
        CommandSpec {
            program: "pnpm",
            arguments: &["check:eslint"],
        },
        CommandSpec {
            program: "pnpm",
            arguments: &["check:prettier"],
        },
        CommandSpec {
            program: "pnpm",
            arguments: &["check:markdown"],
        },
        CommandSpec {
            program: "pnpm",
            arguments: &["check:links"],
        },
        CommandSpec {
            program: "pnpm",
            arguments: &["test:unit"],
        },
        CommandSpec {
            program: "pnpm",
            arguments: &["build"],
        },
    ]
}

fn run_commands(
    root: &Path,
    runner: &mut impl Runner,
    commands: &[CommandSpec],
) -> Result<(), TaskError> {
    for command in commands {
        runner.run(root, command)?;
    }
    Ok(())
}

fn require_path(root: &Path, relative_path: &str) -> Result<(), TaskError> {
    let path = root.join(relative_path);
    if path.exists() {
        Ok(())
    } else {
        Err(TaskError::MissingPath(path))
    }
}

fn run_codegen(root: &Path, runner: &mut impl Runner, check: bool) -> Result<(), TaskError> {
    require_path(root, "proto/buf.yaml")?;
    require_path(root, "proto/buf.gen.yaml")?;
    runner.run(
        root,
        &CommandSpec {
            program: "node",
            arguments: if check {
                &["tooling/generate-protocol.mjs", "--check"]
            } else {
                &["tooling/generate-protocol.mjs", "--write"]
            },
        },
    )
}

fn run_fixtures(root: &Path, runner: &mut impl Runner) -> Result<(), TaskError> {
    require_path(root, "tests/fixtures/manifest.json")?;
    runner.run(
        root,
        &CommandSpec {
            program: "cargo",
            arguments: &[
                "test",
                "-p",
                "tether-protocol",
                "--test",
                "fixture_generation",
                "--",
                "--ignored",
                "--exact",
                "generate_fixtures",
            ],
        },
    )
}

fn package_preflight(root: &Path) -> Result<(), TaskError> {
    for required in [
        "AGENTS.md",
        "Cargo.lock",
        "Cargo.toml",
        "SPEC.md",
        "apps/desktop/src-tauri/Cargo.toml",
        "package.json",
        "pnpm-lock.yaml",
        "rust-toolchain.toml",
    ] {
        require_path(root, required)?;
    }
    Ok(())
}

fn manual_test_guard(environment: &impl Environment, scope: &str) -> Result<(), TaskError> {
    let acknowledgement = environment
        .value("TETHER_MANUAL_TEST_ACK")
        .and_then(|value| value.into_string().ok());
    if acknowledgement.as_deref() != Some(MANUAL_TEST_ACKNOWLEDGEMENT) {
        return Err(TaskError::MissingEnvironment("TETHER_MANUAL_TEST_ACK"));
    }

    let target = environment
        .value("TETHER_MANUAL_TEST_TARGET")
        .and_then(|value| value.into_string().ok());
    if target.as_deref().is_none_or(str::is_empty) {
        return Err(TaskError::MissingEnvironment("TETHER_MANUAL_TEST_TARGET"));
    }

    println!("manual-test guard satisfied for scope {scope:?}; no native test was run");
    Ok(())
}

fn print_help() {
    println!(
        "\
Tether repository tasks

  cargo xtask check
  cargo xtask codegen
  cargo xtask check-generated
  cargo xtask fixtures
  cargo xtask package-preflight
  cargo xtask manual-test-guard <scope>

The aggregate check never runs native input injection. Native/manual tests must
independently pass the explicit acknowledgement and named-target guard."
    );
}

fn execute(
    task: &Task,
    root: &Path,
    environment: &impl Environment,
    runner: &mut impl Runner,
) -> Result<(), TaskError> {
    match task {
        Task::Check => run_commands(root, runner, check_commands()),
        Task::Codegen => run_codegen(root, runner, false),
        Task::CheckGenerated => run_codegen(root, runner, true),
        Task::CodegenRust { descriptor, output } => protocol_codegen::generate(descriptor, output)
            .map_err(|_| TaskError::InvalidArguments("Rust protocol generation failed".to_owned())),
        Task::Fixtures => run_fixtures(root, runner),
        Task::Help => {
            print_help();
            Ok(())
        }
        Task::ManualTestGuard { scope } => manual_test_guard(environment, scope),
        Task::PackagePreflight => package_preflight(root),
    }
}

fn main() -> ExitCode {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
    let result = parse_task(&arguments).and_then(|task| {
        let current_directory = env::current_dir().map_err(|error| TaskError::Spawn {
            program: "current_dir".to_owned(),
            message: error.to_string(),
        })?;
        let root = find_workspace_root(&current_directory)?;
        execute(&task, &root, &ProcessEnvironment, &mut ProcessRunner)
    });

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("xtask: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[derive(Default)]
    struct FakeEnvironment {
        values: HashMap<String, OsString>,
    }

    impl Environment for FakeEnvironment {
        fn value(&self, name: &str) -> Option<OsString> {
            self.values.get(name).cloned()
        }
    }

    #[derive(Default)]
    struct RecordingRunner {
        commands: Vec<CommandSpec>,
    }

    impl Runner for RecordingRunner {
        fn run(&mut self, _root: &Path, specification: &CommandSpec) -> Result<(), TaskError> {
            self.commands.push(specification.clone());
            Ok(())
        }
    }

    fn arguments(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn no_arguments_selects_help() {
        assert_eq!(parse_task(&[]), Ok(Task::Help));
    }

    #[test]
    fn parses_all_fixed_commands() {
        assert_eq!(parse_task(&arguments(&["check"])), Ok(Task::Check));
        assert_eq!(parse_task(&arguments(&["codegen"])), Ok(Task::Codegen));
        assert_eq!(
            parse_task(&arguments(&["check-generated"])),
            Ok(Task::CheckGenerated)
        );
        assert_eq!(parse_task(&arguments(&["fixtures"])), Ok(Task::Fixtures));
        assert_eq!(
            parse_task(&arguments(&["package-preflight"])),
            Ok(Task::PackagePreflight)
        );
    }

    #[test]
    fn parses_manual_guard_scope() {
        assert_eq!(
            parse_task(&arguments(&["manual-test-guard", "macos-input"])),
            Ok(Task::ManualTestGuard {
                scope: "macos-input".to_owned()
            })
        );
    }

    #[test]
    fn rejects_unknown_and_extra_arguments() {
        assert!(matches!(
            parse_task(&arguments(&["unknown"])),
            Err(TaskError::InvalidArguments(_))
        ));
        assert!(matches!(
            parse_task(&arguments(&["check", "extra"])),
            Err(TaskError::InvalidArguments(_))
        ));
        assert!(matches!(
            parse_task(&arguments(&["manual-test-guard"])),
            Err(TaskError::InvalidArguments(_))
        ));
    }

    #[test]
    fn aggregate_check_has_no_native_test_command() {
        let commands = check_commands();
        assert_ne!(commands, []);
        assert!(commands.iter().all(|command| {
            !command
                .arguments
                .iter()
                .any(|argument| argument.contains("native"))
        }));
    }

    #[test]
    fn runner_stops_on_first_error() {
        struct FailingRunner {
            calls: usize,
        }

        impl Runner for FailingRunner {
            fn run(&mut self, _root: &Path, specification: &CommandSpec) -> Result<(), TaskError> {
                self.calls += 1;
                Err(TaskError::CommandFailed {
                    program: specification.program.to_owned(),
                    status: Some(9),
                })
            }
        }

        let mut runner = FailingRunner { calls: 0 };
        assert!(run_commands(Path::new("."), &mut runner, check_commands()).is_err());
        assert_eq!(runner.calls, 1);
    }

    #[test]
    fn manual_guard_requires_exact_acknowledgement_and_target() {
        let mut environment = FakeEnvironment::default();
        assert_eq!(
            manual_test_guard(&environment, "test"),
            Err(TaskError::MissingEnvironment("TETHER_MANUAL_TEST_ACK"))
        );

        environment.values.insert(
            "TETHER_MANUAL_TEST_ACK".to_owned(),
            OsString::from(MANUAL_TEST_ACKNOWLEDGEMENT),
        );
        assert_eq!(
            manual_test_guard(&environment, "test"),
            Err(TaskError::MissingEnvironment("TETHER_MANUAL_TEST_TARGET"))
        );

        environment.values.insert(
            "TETHER_MANUAL_TEST_TARGET".to_owned(),
            OsString::from("isolated-lab"),
        );
        assert_eq!(manual_test_guard(&environment, "test"), Ok(()));
    }

    #[test]
    fn finds_real_workspace_and_passes_current_package_preflight() {
        let manifest_directory = Path::new(env!("CARGO_MANIFEST_DIR"));
        let root = find_workspace_root(manifest_directory).expect("workspace root must exist");
        assert_eq!(
            root,
            manifest_directory.parent().expect("xtask has a parent")
        );
        assert_eq!(package_preflight(&root), Ok(()));
    }

    #[test]
    fn codegen_fails_before_spawning_when_configuration_is_missing() {
        let mut runner = RecordingRunner::default();
        let missing_root = Path::new("/definitely/not/a/tether/workspace");
        assert!(matches!(
            run_codegen(missing_root, &mut runner, false),
            Err(TaskError::MissingPath(_))
        ));
        assert_eq!(runner.commands, Vec::<CommandSpec>::new());
    }

    #[test]
    fn package_preflight_reports_missing_required_path() {
        let missing_root = Path::new("/definitely/not/a/tether/workspace");
        assert!(matches!(
            package_preflight(missing_root),
            Err(TaskError::MissingPath(_))
        ));
    }
}

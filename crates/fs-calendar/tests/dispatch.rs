//! The first word picks the subcommand (ADR 0063).

use std::process::Command;

fn run(args: &[&str]) -> (Option<i32>, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_fs-calendar"))
        .args(args)
        .output()
        .unwrap();
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn help_names_all_three_ways_in() {
    let (code, out, _) = run(&["--help"]);
    assert_eq!(code, Some(0));
    for word in ["fs-calendar serve", "fs-calendar app", "list", "import"] {
        assert!(out.contains(word), "{word} missing in {out}");
    }
}

#[test]
fn an_unknown_command_is_refused_with_2() {
    let (code, _, err) = run(&["frob"]);
    assert_eq!(code, Some(2));
    assert!(err.contains("unknown command frob"), "{err}");
}

#[test]
fn serve_help_is_the_servers_own() {
    let (code, out, _) = run(&["serve", "--help"]);
    assert_eq!(code, Some(0));
    assert!(
        out.contains("--socket") && out.contains("--instance"),
        "{out}"
    );
}

#[test]
fn a_command_reaches_a_missing_server_with_exit_1() {
    let dir = std::env::temp_dir().join("fs-calendar-no-such.sock");
    let (code, _, err) = run(&["--socket", dir.to_str().unwrap(), "list"]);
    assert_eq!(code, Some(1));
    assert!(err.contains("fs-calendar:"), "{err}");
}

#[cfg(not(feature = "gui"))]
#[test]
fn without_the_gui_the_bare_command_prints_help() {
    let (code, out, _) = run(&[]);
    assert_eq!(code, Some(0));
    assert!(out.contains("fs-calendar serve"), "{out}");
}

#[test]
fn version_names_the_program_and_the_workspace_version_and_needs_nothing() {
    for flag in ["--version", "-V"] {
        let (code, out, err) = run(&[flag]);
        assert_eq!(code, Some(0), "{err}");
        assert_eq!(
            out.trim(),
            format!("fs-calendar {}", env!("CARGO_PKG_VERSION"))
        );
    }
    // the version is that of the workspace, in the form 0.0.N (ADR 0064)
    let parts: Vec<&str> = env!("CARGO_PKG_VERSION").split('.').collect();
    assert_eq!((parts.len(), parts[0], parts[1]), (3, "0", "0"));
}

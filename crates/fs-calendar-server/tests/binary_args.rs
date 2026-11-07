//! The real program refuses what it should refuse, and starts nothing.

use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};

#[test]
fn it_refuses_an_instance_that_is_no_domain() {
    let dir = tempfile::tempdir().unwrap();
    for bad in [
        "a/b",
        "x y",
        "user@example.org",
        "example.org:80",
        "-a.example",
    ] {
        let code = Command::new(env!("CARGO_BIN_EXE_fs-calendar-server"))
            .arg("--socket")
            .arg(dir.path().join("x.sock"))
            .arg("--data")
            .arg(dir.path().join("data"))
            .args(["--instance", bad])
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .code();
        assert_eq!(code, Some(2), "{bad}");
    }
    assert!(!dir.path().join("data").exists(), "nothing was opened");
}

#[test]
fn it_refuses_bad_arguments_and_a_data_directory_others_can_read() {
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_fs-calendar-server"))
            .args(args)
            .stderr(Stdio::null())
            .stdout(Stdio::null())
            .status()
            .unwrap()
            .code()
    };
    assert_eq!(run(&["--help"]), Some(0));
    assert_eq!(run(&["--nonsense"]), Some(2));
    assert_eq!(run(&["--socket"]), Some(2));
    assert_eq!(
        run(&["--socket", "/s", "--data", "/d", "--actor", ""]),
        Some(2)
    );

    let dir = tempfile::tempdir().unwrap();
    let open = dir.path().join("open");
    std::fs::create_dir(&open).unwrap();
    std::fs::set_permissions(&open, PermissionsExt::from_mode(0o755)).unwrap();
    let socket = dir.path().join("x.sock");
    let (socket, open) = (socket.to_str().unwrap(), open.to_str().unwrap());
    assert_eq!(run(&["--socket", socket, "--data", open]), Some(1));
}

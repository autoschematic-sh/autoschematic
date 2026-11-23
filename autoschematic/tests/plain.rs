use std::process::{Command, Output, Stdio};

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_autoschematic"))
        .args(args)
        .stdin(Stdio::null())
        .env("CLICOLOR_FORCE", "1")
        .output()
        .unwrap()
}

fn assert_plain(output: &Output) {
    for stream in [&output.stdout, &output.stderr] {
        assert!(
            !stream.contains(&0x1b),
            "ANSI escape in output: {}",
            String::from_utf8_lossy(stream)
        );
        assert!(!stream.contains(&b'\r'), "Carriage return in output");
    }
}

#[test]
fn plain_flag_works_before_and_after_subcommands() {
    for args in [
        vec!["--plain", "version"],
        vec!["version", "--plain"],
        vec!["--plain", "--help"],
    ] {
        let output = cli(&args);
        assert!(output.status.success());
        assert_plain(&output);
    }
}

#[test]
fn redirected_output_is_plain_and_version_flag_works() {
    let output = cli(&["--version"]);
    assert!(output.status.success());
    assert_plain(&output);
    assert!(String::from_utf8_lossy(&output.stdout).contains(env!("CARGO_PKG_VERSION")));
}

use std::process::{Command, Stdio};

#[test]
fn plan_and_apply_filters_prevent_unselected_connectors_from_starting() {
    let directory = tempfile::tempdir().unwrap();
    let repository = git2::Repository::init(directory.path()).unwrap();
    std::fs::write(
        directory.path().join("autoschematic.ron"),
        r#"AutoschematicConfig(prefixes: {
            "prod": Prefix(connectors: [Connector(shortname: "test", spec: Cargo(name: "missing-test-connector"))]),
            "dev": Prefix(connectors: [
                Connector(shortname: "unrelated", spec: Cargo(name: "missing-unrelated-connector")),
                Connector(shortname: "test", spec: Cargo(name: "missing-test-connector")),
            ]),
        })"#,
    )
    .unwrap();
    std::fs::create_dir_all(directory.path().join("dev/test")).unwrap();
    std::fs::write(directory.path().join("dev/test/record.ron"), "Record()").unwrap();
    let mut index = repository.index().unwrap();
    index.add_path(std::path::Path::new("dev/test/record.ron")).unwrap();
    index.write().unwrap();

    for subcommand in ["plan", "apply", "unbundle"] {
        for filter in [["--prefix", "prod"], ["--subpath", "test/other.ron"]] {
            let output = Command::new(env!("CARGO_BIN_EXE_autoschematic"))
                .env("AUTOSCHEMATIC_NO_MOTD", "1")
                .current_dir(directory.path())
                .arg(subcommand)
                .args(filter)
                .stdin(Stdio::null())
                .output()
                .unwrap();
            assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
            assert!(String::from_utf8_lossy(&output.stdout).contains("No staged files match"));
        }
    }

    let output = Command::new(env!("CARGO_BIN_EXE_autoschematic"))
        .env("AUTOSCHEMATIC_NO_MOTD", "1")
        .current_dir(directory.path())
        .args(["plan", "--prefix", "typo"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Unknown prefix: typo"));

    for subcommand in ["plan", "apply", "unbundle"] {
        let output = Command::new(env!("CARGO_BIN_EXE_autoschematic"))
            .env("AUTOSCHEMATIC_NO_MOTD", "1")
            .current_dir(directory.path())
            .args([subcommand, "--prefix", "dev", "--connector", "typo"])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("Unknown connector"));
    }

    let output = Command::new(env!("CARGO_BIN_EXE_autoschematic"))
        .env("AUTOSCHEMATIC_NO_MOTD", "1")
        .current_dir(directory.path())
        .args(["plan", "--prefix", "dev", "--subpath", "test/*.ron", "--connector", "test"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("missing-test-connector"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output = Command::new(env!("CARGO_BIN_EXE_autoschematic"))
        .env("AUTOSCHEMATIC_NO_MOTD", "1")
        .current_dir(directory.path())
        .args(["check-drift", "--path", "dev/test/record.ron", "--connector", "test"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("missing-test-connector"), "{stderr}");
    assert!(!stderr.contains("missing-unrelated-connector"));
}

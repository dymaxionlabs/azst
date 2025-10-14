// Integration tests for azst CLI tool
//
// These tests verify CLI behavior, argument parsing, and error handling
// without requiring actual Azure CLI access.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

#[cfg(test)]
mod cli_parsing_tests {
    use super::*;

    #[test]
    fn test_cli_help() {
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.arg("--help");
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("Rust CLI tool"));
    }

    #[test]
    fn test_cli_version() {
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.arg("--version");
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("azst"));
    }

    #[test]
    fn test_cli_no_command() {
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.assert()
            .failure()
            .stderr(predicate::str::contains("Usage"));
    }

    #[test]
    fn test_cli_invalid_command() {
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.arg("invalid-command");
        cmd.assert()
            .failure()
            .stderr(predicate::str::contains("unrecognized subcommand"));
    }
}

#[cfg(test)]
mod cp_command_tests {
    use super::*;

    #[test]
    fn test_cp_missing_args() {
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.arg("cp");
        cmd.assert()
            .failure()
            .stderr(predicate::str::contains("required arguments"));
    }

    #[test]
    fn test_cp_help() {
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["cp", "--help"]);
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("Copy files"));
    }

    #[test]
    fn test_cp_local_to_local() {
        let temp_dir = TempDir::new().unwrap();
        let source_file = temp_dir.path().join("source.txt");
        let dest_file = temp_dir.path().join("dest.txt");

        // Create source file
        fs::write(&source_file, "test content").unwrap();

        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&[
            "cp",
            source_file.to_str().unwrap(),
            dest_file.to_str().unwrap(),
        ]);

        cmd.assert().success();

        // Verify file was copied
        assert!(dest_file.exists());
        let content = fs::read_to_string(&dest_file).unwrap();
        assert_eq!(content, "test content");
    }

    #[test]
    fn test_cp_recursive_flag() {
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["cp", "-r", "--help"]);
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("recursive"));
    }

    #[test]
    fn test_cp_parallel_flag() {
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["cp", "-j", "8", "--help"]);
        cmd.assert().success();
    }

    #[test]
    fn test_cp_nonexistent_source() {
        let temp_dir = TempDir::new().unwrap();
        let dest_file = temp_dir.path().join("dest.txt");

        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["cp", "/nonexistent/file.txt", dest_file.to_str().unwrap()]);

        cmd.assert()
            .failure()
            .stderr(predicate::str::contains("No such file or directory").or(
                predicate::str::contains("The system cannot find the path specified"),
            ));
    }
}

#[cfg(test)]
mod ls_command_tests {
    use super::*;

    #[test]
    fn test_ls_help() {
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["ls", "--help"]);
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("List objects"));
    }

    #[test]
    fn test_ls_local_directory() {
        let temp_dir = TempDir::new().unwrap();

        // Create some test files
        fs::write(temp_dir.path().join("file1.txt"), "content1").unwrap();
        fs::write(temp_dir.path().join("file2.txt"), "content2").unwrap();

        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["ls", temp_dir.path().to_str().unwrap()]);

        cmd.assert()
            .success()
            .stdout(predicate::str::contains("file1.txt"))
            .stdout(predicate::str::contains("file2.txt"));
    }

    #[test]
    fn test_ls_local_file() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "content").unwrap();

        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["ls", test_file.to_str().unwrap()]);

        cmd.assert()
            .success()
            .stdout(predicate::str::contains("test.txt"));
    }

    #[test]
    fn test_ls_long_format() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("test.txt"), "content").unwrap();

        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["ls", "-l", temp_dir.path().to_str().unwrap()]);

        cmd.assert()
            .success()
            .stdout(predicate::str::contains("Size"));
    }

    #[test]
    fn test_ls_human_readable() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("test.txt"), "x".repeat(2048)).unwrap();

        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["ls", "-lH", temp_dir.path().to_str().unwrap()]);

        cmd.assert()
            .success()
            .stdout(predicate::str::contains("KB").or(predicate::str::contains("B")));
    }

    #[test]
    fn test_ls_nonexistent_path() {
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["ls", "/nonexistent/path"]);

        cmd.assert()
            .failure()
            .stderr(predicate::str::contains("does not exist"));
    }
}

#[cfg(test)]
mod rm_command_tests {
    use super::*;

    #[test]
    fn test_rm_help() {
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["rm", "--help"]);
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("Remove objects"));
    }

    #[test]
    fn test_rm_missing_path() {
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.arg("rm");
        cmd.assert()
            .failure()
            .stderr(predicate::str::contains("required"));
    }

    #[test]
    fn test_rm_recursive_flag() {
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["rm", "-r", "--help"]);
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("recursive"));
    }

    #[test]
    fn test_rm_force_flag() {
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["rm", "-f", "--help"]);
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("force"));
    }

    #[test]
    fn test_rm_nonexistent_file() {
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["rm", "-f", "/nonexistent/file.txt"]);

        cmd.assert()
            .failure()
            .stderr(predicate::str::contains("does not exist"));
    }
}

#[cfg(test)]
mod mb_command_tests {
    use super::*;

    #[test]
    fn test_mb_help() {
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["mb", "--help"]);
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("Make bucket"));
    }

    #[test]
    fn test_mb_missing_container() {
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.arg("mb");
        cmd.assert()
            .failure()
            .stderr(predicate::str::contains("required"));
    }

    #[test]
    fn test_mb_invalid_uri() {
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["mb", "/local/path"]);
        cmd.assert()
            .failure()
            .stderr(predicate::str::contains("az://"));
    }

    #[test]
    fn test_mb_with_path_error() {
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["mb", "az://account/container/path"]);
        cmd.assert()
            .failure()
            .stderr(predicate::str::contains("Cannot specify path"));
    }
}

#[cfg(test)]
mod rb_command_tests {
    use super::*;

    #[test]
    fn test_rb_help() {
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["rb", "--help"]);
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("Remove bucket"));
    }

    #[test]
    fn test_rb_missing_container() {
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.arg("rb");
        cmd.assert()
            .failure()
            .stderr(predicate::str::contains("required"));
    }

    #[test]
    fn test_rb_invalid_uri() {
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["rb", "/local/path"]);
        cmd.assert()
            .failure()
            .stderr(predicate::str::contains("az://"));
    }

    #[test]
    fn test_rb_with_path_error() {
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["rb", "az://account/container/path"]);
        cmd.assert()
            .failure()
            .stderr(predicate::str::contains("Cannot specify path"));
    }

    #[test]
    fn test_rb_force_flag() {
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["rb", "-f", "--help"]);
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("force"));
    }
}

#[cfg(test)]
mod utils_integration_tests {
    use super::*;

    #[test]
    fn test_uri_validation_in_commands() {
        // Test that invalid URIs result in file not found error
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["ls", "invalid://uri"]);
        cmd.assert()
            .failure()
            .stderr(predicate::str::contains("does not exist"));
    }

    #[test]
    fn test_azure_uri_format_in_help() {
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["cp", "--help"]);
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("az://"));
    }
}

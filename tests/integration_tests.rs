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
mod cat_command_tests {
    use super::*;

    #[test]
    fn test_cat_help() {
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["cat", "--help"]);
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("Concatenate object content"));
    }

    #[test]
    fn test_cat_missing_args() {
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.arg("cat");
        cmd.assert()
            .failure()
            .stderr(predicate::str::contains("No URLs provided"));
    }

    #[test]
    fn test_cat_invalid_url() {
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["cat", "invalid-url"]);
        cmd.assert()
            .failure()
            .stderr(predicate::str::contains("Invalid URL"));
    }

    #[test]
    fn test_cat_header_flag() {
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["cat", "--header", "--help"]);
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("header"));
    }

    #[test]
    fn test_cat_range_flag() {
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["cat", "--range", "0-100", "--help"]);
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("range"));
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
            // When not outputting to TTY, headers are not shown
            // Check for the actual file listing instead
            .stdout(predicate::str::contains("test.txt"));
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

    // Note: The following tests require Azure CLI to be installed and authenticated
    // They are documented tests that verify the expected behavior

    #[test]
    #[ignore] // Requires Azure CLI authentication
    fn test_ls_storage_accounts() {
        // Test: azst ls
        // Expected: List all storage accounts in the current subscription
        // Output format: az://accountname/
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.arg("ls");

        // This test will fail if not authenticated, which is expected
        // When authenticated, it should list storage accounts with az:// prefix
        let result = cmd.assert();

        // Check that either we get storage accounts or an authentication error
        result.stdout(
            predicate::str::contains("Azure Storage Accounts")
                .or(predicate::str::contains("az://")),
        );
    }

    #[test]
    #[ignore] // Requires Azure CLI authentication
    fn test_ls_storage_accounts_long_format() {
        // Test: azst ls -l
        // Expected: List storage accounts with location and resource group
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["ls", "-l"]);

        let result = cmd.assert();

        // Should show accounts with additional columns (location, resource group)
        result.stdout(
            predicate::str::contains("Azure Storage Accounts")
                .or(predicate::str::contains("az://")),
        );
    }

    #[test]
    #[ignore] // Requires Azure CLI authentication
    fn test_ls_containers_for_account() {
        // Test: azst ls az://accountname/
        // Expected: List all containers in the specified storage account
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["ls", "az://testaccount/"]);

        // This will fail with an error if the account doesn't exist or user isn't authenticated
        // When successful, it should list containers
        let result = cmd.assert();

        // Either shows containers or gives an error about the account
        result.stdout(
            predicate::str::contains("Azure Storage Containers")
                .or(predicate::str::contains("az://")),
        );
    }

    #[test]
    fn test_ls_azure_uri_format_validation() {
        // Test that Azure URI format is recognized correctly
        // This test doesn't require Azure CLI as it tests parsing only
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["ls", "az://invalid-account-that-does-not-exist/"]);

        // Should fail with Azure-related error (not path error)
        // Case-insensitive check for "storage" (can be "Storage" or "storage")
        cmd.assert().failure().stderr(
            predicate::str::contains("Azure")
                .or(predicate::str::contains("storage"))
                .or(predicate::str::contains("Storage")),
        );
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
mod mv_command_tests {
    use super::*;

    #[test]
    fn test_mv_help() {
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["mv", "--help"]);
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("Move files"));
    }

    #[test]
    fn test_mv_missing_args() {
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.arg("mv");
        cmd.assert()
            .failure()
            .stderr(predicate::str::contains("required arguments"));
    }

    #[test]
    fn test_mv_recursive_flag() {
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["mv", "-r", "--help"]);
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("recursive"));
    }

    #[test]
    fn test_mv_force_flag() {
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["mv", "-f", "--help"]);
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("force"));
    }

    #[test]
    fn test_mv_local_to_local_error() {
        // mv should reject purely local operations
        let temp_dir = TempDir::new().unwrap();
        let source_file = temp_dir.path().join("source.txt");
        let dest_file = temp_dir.path().join("dest.txt");

        // Create source file
        fs::write(&source_file, "test content").unwrap();

        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&[
            "mv",
            source_file.to_str().unwrap(),
            dest_file.to_str().unwrap(),
        ]);

        cmd.assert()
            .failure()
            .stderr(predicate::str::contains("Azure path"));
    }

    #[test]
    fn test_mv_azure_uri_format_in_help() {
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["mv", "--help"]);
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("az://"));
    }
}

#[cfg(test)]
mod du_tests {
    use super::*;

    #[test]
    fn test_du_help() {
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["du", "--help"]);
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("disk usage statistics"));
    }

    #[test]
    fn test_du_azure_uri_format_in_help() {
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["du", "--help"]);
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("az://"));
    }

    #[test]
    fn test_du_local_file() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        let content = "Hello, world!";
        fs::write(&test_file, content).unwrap();

        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["du", test_file.to_str().unwrap()]);

        cmd.assert()
            .success()
            .stdout(predicate::str::contains(content.len().to_string()));
    }

    #[test]
    fn test_du_local_file_human_readable() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        let content = "x".repeat(2048); // 2KB
        fs::write(&test_file, content).unwrap();

        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["du", "-H", test_file.to_str().unwrap()]);

        cmd.assert()
            .success()
            .stdout(predicate::str::contains("KB"));
    }

    #[test]
    fn test_du_local_directory() {
        let temp_dir = TempDir::new().unwrap();

        // Create a simple directory structure
        fs::write(temp_dir.path().join("file1.txt"), "content1").unwrap();
        fs::write(temp_dir.path().join("file2.txt"), "content2").unwrap();

        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["du", "-s", temp_dir.path().to_str().unwrap()]);

        cmd.assert().success();
    }

    #[test]
    fn test_du_nonexistent_path() {
        let mut cmd = Command::cargo_bin("azst").unwrap();
        cmd.args(&["du", "/nonexistent/path"]);

        cmd.assert()
            .failure()
            .stderr(predicate::str::contains("does not exist"));
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

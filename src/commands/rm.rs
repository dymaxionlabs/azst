use anyhow::{anyhow, Result};
use colored::*;
use std::io::{self, Write};

use crate::azure::{convert_az_uri_to_url, AzCopyClient};
use crate::utils::{is_azure_uri, parse_azure_uri};

pub async fn execute(path: &str, recursive: bool, force: bool) -> Result<()> {
    if is_azure_uri(path) {
        let azcopy = AzCopyClient::new();
        azcopy.check_prerequisites().await?;
        remove_azure_object(path, recursive, force).await
    } else {
        remove_local_path(path, recursive, force).await
    }
}

async fn remove_azure_object(path: &str, recursive: bool, force: bool) -> Result<()> {
    let (_account, container, blob_path) = parse_azure_uri(path)?;

    // Validate that we have a container specified
    if container.is_empty() {
        return Err(anyhow!(
            "Invalid URI '{}'. You must specify both storage account and container: az://<account>/<container>/[path]",
            path
        ));
    }

    // Check if trying to remove entire container
    if blob_path.is_none() {
        return Err(anyhow!(
            "Cannot remove entire container with rm. Use 'azst rb' instead"
        ));
    }

    // Prompt for confirmation unless force flag is set
    if !force {
        let action = if recursive {
            "recursively remove"
        } else {
            "remove"
        };
        print!("{} {}? (y/N): ", action, path.yellow());
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim().to_lowercase();

        if input != "y" && input != "yes" {
            println!("Aborted");
            return Ok(());
        }
    }

    // Convert az:// URI to HTTPS URL for AzCopy
    let target_url = convert_az_uri_to_url(path)?;

    let op_type = if recursive {
        "Removing (recursive)"
    } else {
        "Removing"
    };
    println!("{} {} {}", "×".red(), op_type, path.cyan());

    // Show the actual AzCopy command for debugging
    let recursive_flag = if recursive { " --recursive" } else { "" };
    println!(
        "{} {}",
        "⚙".dimmed(),
        format!("azcopy remove '{}'{}", target_url, recursive_flag).dimmed()
    );
    println!(); // Blank line before AzCopy output

    // Use AzCopy for removal
    let azcopy = AzCopyClient::new();
    azcopy.remove(&target_url, recursive).await?;

    println!(); // Blank line after AzCopy output
    println!("{} Removed successfully", "✓".green());
    Ok(())
}

async fn remove_local_path(path: &str, recursive: bool, force: bool) -> Result<()> {
    use std::path::Path;

    let path_obj = Path::new(path);

    if !path_obj.exists() {
        return Err(anyhow!("Path '{}' does not exist", path));
    }

    if path_obj.is_file() {
        remove_local_file(path, force).await
    } else if path_obj.is_dir() {
        if !recursive {
            return Err(anyhow!("Cannot remove directory without -r flag"));
        }
        remove_local_directory(path, force).await
    } else {
        Err(anyhow!("Path '{}' is neither file nor directory", path))
    }
}

async fn remove_local_file(path: &str, force: bool) -> Result<()> {
    use tokio::fs;

    if !force {
        print!("Remove file '{}'? (y/N): ", path.cyan());
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim().to_lowercase();

        if input != "y" && input != "yes" {
            println!("Aborted");
            return Ok(());
        }
    }

    println!("{} Removing {}", "×".red(), path.cyan());
    fs::remove_file(path).await?;
    println!("{} Removed", "✓".green());

    Ok(())
}

async fn remove_local_directory(path: &str, force: bool) -> Result<()> {
    use tokio::fs;

    if !force {
        print!(
            "Remove directory '{}' and all its contents? (y/N): ",
            path.cyan()
        );
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim().to_lowercase();

        if input != "y" && input != "yes" {
            println!("Aborted");
            return Ok(());
        }
    }

    println!("{} Removing directory {}", "×".red(), path.cyan());
    fs::remove_dir_all(path).await?;
    println!("{} Removed", "✓".green());

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_remove_single_blob_docs() {
        // Test case: azst rm az://account/container/file.txt
        // Expected: Remove single blob after confirmation
    }

    #[test]
    fn test_remove_with_prefix_docs() {
        // Test case: azst rm -r az://account/container/prefix/
        // Expected: Remove all blobs with prefix after confirmation
    }

    #[test]
    fn test_remove_force_docs() {
        // Test case: azst rm -rf az://account/container/prefix/
        // Expected: Remove all blobs with prefix without confirmation
    }

    #[test]
    fn test_remove_local_file_docs() {
        // Test case: azst rm /local/file.txt
        // Expected: Remove local file after confirmation
    }

    #[test]
    fn test_remove_local_directory_docs() {
        // Test case: azst rm -r /local/dir/
        // Expected: Remove local directory recursively after confirmation
    }

    #[test]
    fn test_remove_container_error_docs() {
        // Test case: azst rm az://account/container/
        // Expected: Error - use 'azst rb' instead
    }

    #[test]
    fn test_remove_non_recursive_error_docs() {
        // Test case: azst rm az://account/container/prefix/ (without -r)
        // Expected: Error - requires -r flag
    }
}

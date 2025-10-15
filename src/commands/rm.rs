use anyhow::{anyhow, Result};
use colored::*;
use std::io::{self, Write};

use crate::azure::{convert_az_uri_to_url, AzCopyClient, AzCopyOptions};
use crate::utils::{is_azure_uri, parse_azure_uri};

pub async fn execute(
    path: &str,
    recursive: bool,
    force: bool,
    dry_run: bool,
    include_pattern: Option<&str>,
    exclude_pattern: Option<&str>,
) -> Result<()> {
    if is_azure_uri(path) {
        let mut azcopy = AzCopyClient::new();
        azcopy.check_prerequisites().await?;
        remove_azure_object(
            &mut azcopy,
            path,
            recursive,
            force,
            dry_run,
            include_pattern,
            exclude_pattern,
        )
        .await
    } else {
        remove_local_path(path, recursive, force).await
    }
}

async fn remove_azure_object(
    azcopy: &mut AzCopyClient,
    path: &str,
    recursive: bool,
    force: bool,
    dry_run: bool,
    include_pattern: Option<&str>,
    exclude_pattern: Option<&str>,
) -> Result<()> {
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
        return Err(anyhow!("Cannot remove entire container with rm"));
    }

    // Auto-enable recursive if path contains wildcards
    let has_wildcard = path.contains('*') || path.contains('?');
    let recursive = recursive || has_wildcard;

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

    let mut flags_display = Vec::new();
    if recursive {
        flags_display.push("recursive");
    }
    if dry_run {
        flags_display.push("dry-run");
    }
    if include_pattern.is_some() {
        flags_display.push("filtered");
    }

    let flags_str = if !flags_display.is_empty() {
        format!(" ({})", flags_display.join(", "))
    } else {
        String::new()
    };

    println!(
        "{} Removing {}{}",
        "×".red(),
        path.cyan(),
        flags_str.dimmed()
    );

    // Build options
    let mut options = AzCopyOptions::new()
        .with_recursive(recursive)
        .with_dry_run(dry_run);

    if let Some(pattern) = include_pattern {
        options = options.with_include_pattern(Some(pattern.to_string()));
    }
    if let Some(pattern) = exclude_pattern {
        options = options.with_exclude_pattern(Some(pattern.to_string()));
    }

    // Show the actual AzCopy command for debugging
    let mut cmd_parts = vec![format!("azcopy remove '{}'", target_url)];
    if recursive {
        cmd_parts.push("--recursive".to_string());
    }
    if dry_run {
        cmd_parts.push("--dry-run".to_string());
    }
    if let Some(pattern) = include_pattern {
        cmd_parts.push(format!("--include-pattern='{}'", pattern));
    }
    if let Some(pattern) = exclude_pattern {
        cmd_parts.push(format!("--exclude-pattern='{}'", pattern));
    }
    cmd_parts.push("--output-type json".to_string());

    println!("{} {}", "⚙".dimmed(), cmd_parts.join(" ").dimmed());
    println!(); // Blank line before AzCopy output

    // Use AzCopy for removal
    azcopy.remove_with_options(&target_url, &options).await?;

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

use anyhow::{anyhow, Result};
use colored::*;
use tokio::fs;

use crate::azure::{convert_az_uri_to_url, AzCopyClient, AzCopyOptions};
use crate::utils::{get_filename, get_parent_dir, is_azure_uri, is_directory, path_exists};

pub async fn execute(
    source: &str,
    destination: &str,
    recursive: bool,
    dry_run: bool,
    cap_mbps: Option<f64>,
    include_pattern: Option<&str>,
    exclude_pattern: Option<&str>,
) -> Result<()> {
    let source_is_azure = is_azure_uri(source);
    let dest_is_azure = is_azure_uri(destination);

    match (source_is_azure, dest_is_azure) {
        (false, true) | (true, false) | (true, true) => {
            // Any Azure operation - use AzCopy for performance
            let azcopy = AzCopyClient::new();
            azcopy.check_prerequisites().await?;
            copy_with_azcopy(
                source,
                destination,
                recursive,
                dry_run,
                cap_mbps,
                include_pattern,
                exclude_pattern,
            )
            .await
        }
        (false, false) => {
            // Local to Local - use regular file copy
            copy_local_files(source, destination, recursive).await
        }
    }
}

/// Copy using AzCopy for high performance
async fn copy_with_azcopy(
    source: &str,
    destination: &str,
    recursive: bool,
    dry_run: bool,
    cap_mbps: Option<f64>,
    include_pattern: Option<&str>,
    exclude_pattern: Option<&str>,
) -> Result<()> {
    // Convert az:// URIs to HTTPS URLs for AzCopy
    let source_url = if is_azure_uri(source) {
        convert_az_uri_to_url(source)?
    } else {
        // Validate local path exists
        if !path_exists(source) {
            return Err(anyhow!("Source path '{}' does not exist", source));
        }
        if is_directory(source) && !recursive {
            return Err(anyhow!(
                "Source is a directory. Use -r flag for recursive copy"
            ));
        }
        source.to_string()
    };

    let dest_url = if is_azure_uri(destination) {
        convert_az_uri_to_url(destination)?
    } else {
        destination.to_string()
    };

    // Display operation
    let operation_type = match (is_azure_uri(source), is_azure_uri(destination)) {
        (false, true) => "Uploading",
        (true, false) => "Downloading",
        (true, true) => "Copying (Azure to Azure)",
        _ => "Copying",
    };

    let mut flags_display = Vec::new();
    if recursive {
        flags_display.push("recursive");
    }
    if dry_run {
        flags_display.push("dry-run");
    }
    if cap_mbps.is_some() {
        flags_display.push("rate-limited");
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
        "{} {} {} to {}{}",
        "→".green(),
        operation_type,
        source.cyan(),
        destination.cyan(),
        flags_str.dimmed()
    );

    // Build options
    let mut options = AzCopyOptions::new()
        .with_recursive(recursive)
        .with_dry_run(dry_run)
        .with_cap_mbps(cap_mbps);

    if let Some(pattern) = include_pattern {
        options = options.with_include_pattern(Some(pattern.to_string()));
    }
    if let Some(pattern) = exclude_pattern {
        options = options.with_exclude_pattern(Some(pattern.to_string()));
    }

    // Show the actual AzCopy command for debugging
    let mut cmd_parts = vec![format!("azcopy copy '{}' '{}'", source_url, dest_url)];
    if recursive {
        cmd_parts.push("--recursive".to_string());
    }
    if dry_run {
        cmd_parts.push("--dry-run".to_string());
    }
    if let Some(mbps) = cap_mbps {
        cmd_parts.push(format!("--cap-mbps={}", mbps));
    }
    if let Some(pattern) = include_pattern {
        cmd_parts.push(format!("--include-pattern='{}'", pattern));
    }
    if let Some(pattern) = exclude_pattern {
        cmd_parts.push(format!("--exclude-pattern='{}'", pattern));
    }
    cmd_parts.push("--output-type json".to_string());

    println!("{} {}", "⚙".dimmed(), cmd_parts.join(" ").dimmed());

    // Use AzCopy for the operation
    let azcopy = AzCopyClient::new();
    azcopy
        .copy_with_options(&source_url, &dest_url, &options)
        .await?;

    println!("{} Operation completed successfully", "✓".green());
    Ok(())
}

// Local file operations
async fn copy_local_files(source: &str, destination: &str, recursive: bool) -> Result<()> {
    if is_directory(source) {
        if !recursive {
            return Err(anyhow!(
                "Source is a directory. Use -r flag for recursive copy"
            ));
        }
        copy_directory(source, destination).await
    } else {
        copy_file(source, destination).await
    }
}

async fn copy_file(source: &str, destination: &str) -> Result<()> {
    let dest_path = if is_directory(destination) {
        format!(
            "{}/{}",
            destination.trim_end_matches('/'),
            get_filename(source)
        )
    } else {
        destination.to_string()
    };

    // Create parent directory if needed
    if let Some(parent) = get_parent_dir(&dest_path) {
        fs::create_dir_all(parent).await?;
    }

    println!(
        "{} Copying {} to {}",
        "→".green(),
        source.cyan(),
        dest_path.cyan()
    );

    fs::copy(source, &dest_path).await?;
    println!("{} Copy completed", "✓".green());
    Ok(())
}

fn copy_directory<'a>(
    source: &'a str,
    destination: &'a str,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        let mut entries = fs::read_dir(source).await?;

        // Create destination directory
        fs::create_dir_all(destination).await?;

        while let Some(entry) = entries.next_entry().await? {
            let entry_path = entry.path();
            let entry_str = entry_path.to_str().unwrap();
            let filename = entry.file_name();
            let filename_str = filename.to_str().unwrap();

            let dest_path = format!("{}/{}", destination.trim_end_matches('/'), filename_str);

            if entry_path.is_dir() {
                // Recursively copy subdirectory
                copy_directory(entry_str, &dest_path).await?;
            } else {
                // Copy file
                println!(
                    "{} Copying {} to {}",
                    "→".green(),
                    entry_str.cyan(),
                    dest_path.cyan()
                );

                fs::copy(entry_str, &dest_path).await?;
            }
        }

        Ok(())
    })
}

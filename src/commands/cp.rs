use anyhow::{anyhow, Result};
use colored::*;
use tokio::fs;

use crate::azure::{convert_az_uri_to_url, AzCopyClient};
use crate::utils::{get_filename, get_parent_dir, is_azure_uri, is_directory, path_exists};

pub async fn execute(
    source: &str,
    destination: &str,
    recursive: bool,
    parallel: u32,
) -> Result<()> {
    let source_is_azure = is_azure_uri(source);
    let dest_is_azure = is_azure_uri(destination);

    match (source_is_azure, dest_is_azure) {
        (false, true) | (true, false) | (true, true) => {
            // Any Azure operation - use AzCopy for performance
            let azcopy = AzCopyClient::new();
            azcopy.check_prerequisites().await?;
            copy_with_azcopy(source, destination, recursive, parallel).await
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
    parallel: u32,
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

    println!(
        "{} {} {} to {} {}",
        "→".green(),
        operation_type,
        source.cyan(),
        destination.cyan(),
        if recursive {
            format!("(recursive, {} parallel connections)", parallel).dimmed()
        } else {
            "".dimmed()
        }
    );

    // Show the actual AzCopy command for debugging
    let recursive_flag = if recursive { " --recursive" } else { "" };
    println!(
        "{} {}",
        "⚙".dimmed(),
        format!(
            "azcopy copy '{}' '{}'{} --output-type json",
            source_url, dest_url, recursive_flag
        )
        .dimmed()
    );

    // Use AzCopy for the operation
    let azcopy = AzCopyClient::new();
    azcopy
        .copy(&source_url, &dest_url, recursive, parallel)
        .await?;

    println!(); // Blank line after AzCopy output
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

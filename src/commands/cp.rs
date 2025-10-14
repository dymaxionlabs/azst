use anyhow::{anyhow, Result};
use colored::*;
use tokio::fs;

use crate::azure::AzureClient;
use crate::utils::{
    get_filename, get_parent_dir, is_azure_uri, is_directory, parse_azure_uri, path_exists,
};

pub async fn execute(
    source: &str,
    destination: &str,
    recursive: bool,
    parallel: u32,
) -> Result<()> {
    let azure_client = AzureClient::new();
    azure_client.check_prerequisites().await?;

    let source_is_azure = is_azure_uri(source);
    let dest_is_azure = is_azure_uri(destination);

    match (source_is_azure, dest_is_azure) {
        (false, true) => {
            // Local to Azure
            upload_to_azure(source, destination, recursive, parallel, &azure_client).await
        }
        (true, false) => {
            // Azure to Local
            download_from_azure(source, destination, recursive, parallel, &azure_client).await
        }
        (true, true) => {
            // Azure to Azure (not implemented yet)
            Err(anyhow!("Azure to Azure copy is not yet implemented"))
        }
        (false, false) => {
            // Local to Local - just use regular file copy
            copy_local_files(source, destination, recursive).await
        }
    }
}

async fn upload_to_azure(
    source: &str,
    destination: &str,
    recursive: bool,
    _parallel: u32,
    azure_client: &AzureClient,
) -> Result<()> {
    let (account, dest_container, dest_path) = parse_azure_uri(destination)?;

    // Create azure client with account if specified in URI
    let client = if let Some(account_name) = account {
        AzureClient::new().with_storage_account(&account_name)
    } else {
        azure_client.clone()
    };

    if !path_exists(source) {
        return Err(anyhow!("Source path '{}' does not exist", source));
    }

    if is_directory(source) {
        if !recursive {
            return Err(anyhow!(
                "Source is a directory. Use -r flag for recursive copy"
            ));
        }

        // Upload directory recursively
        upload_directory(source, &dest_container, dest_path.as_deref(), &client).await
    } else {
        // Upload single file
        let blob_name = if let Some(path) = dest_path {
            if path.ends_with('/') {
                format!("{}{}", path, get_filename(source))
            } else {
                path
            }
        } else {
            get_filename(source)
        };

        println!(
            "{} Uploading {} to az://{}/{}",
            "→".green(),
            source.cyan(),
            dest_container.yellow(),
            blob_name.cyan()
        );

        client
            .upload_file(source, &dest_container, &blob_name)
            .await?;
        println!("{} Upload completed", "✓".green());
        Ok(())
    }
}

fn upload_directory<'a>(
    dir_path: &'a str,
    container: &'a str,
    base_path: Option<&'a str>,
    azure_client: &'a AzureClient,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        let mut entries = fs::read_dir(dir_path).await?;

        while let Some(entry) = entries.next_entry().await? {
            let entry_path = entry.path();
            let entry_str = entry_path.to_str().unwrap();
            let filename = entry.file_name();
            let filename_str = filename.to_str().unwrap();

            let blob_name = if let Some(base) = base_path {
                format!("{}/{}", base.trim_end_matches('/'), filename_str)
            } else {
                filename_str.to_string()
            };

            if entry_path.is_dir() {
                // Recursively upload subdirectory
                upload_directory(entry_str, container, Some(&blob_name), azure_client).await?;
            } else {
                // Upload file
                println!(
                    "{} Uploading {} to az://{}/{}",
                    "→".green(),
                    entry_str.cyan(),
                    container.yellow(),
                    blob_name.cyan()
                );

                azure_client
                    .upload_file(entry_str, container, &blob_name)
                    .await?;
            }
        }

        Ok(())
    })
}

async fn download_from_azure(
    source: &str,
    destination: &str,
    recursive: bool,
    _parallel: u32,
    azure_client: &AzureClient,
) -> Result<()> {
    let (account, source_container, source_path) = parse_azure_uri(source)?;

    // Create azure client with account if specified in URI
    let client = if let Some(account_name) = account {
        AzureClient::new().with_storage_account(&account_name)
    } else {
        azure_client.clone()
    };

    if let Some(path) = source_path {
        if path.ends_with('/') || recursive {
            // Download directory/prefix
            download_directory(&source_container, Some(&path), destination, &client).await
        } else {
            // Download single blob
            let dest_path = if is_directory(destination) {
                format!(
                    "{}/{}",
                    destination.trim_end_matches('/'),
                    get_filename(&path)
                )
            } else {
                destination.to_string()
            };

            // Create parent directory if it doesn't exist
            if let Some(parent) = get_parent_dir(&dest_path) {
                fs::create_dir_all(parent).await?;
            }

            println!(
                "{} Downloading az://{}/{} to {}",
                "←".blue(),
                source_container.yellow(),
                path.cyan(),
                dest_path.cyan()
            );

            client
                .download_file(&source_container, &path, &dest_path)
                .await?;
            println!("{} Download completed", "✓".green());
            Ok(())
        }
    } else {
        // Download entire container
        if !recursive {
            return Err(anyhow!("Downloading entire container requires -r flag"));
        }
        download_directory(&source_container, None, destination, &client).await
    }
}

async fn download_directory(
    container: &str,
    prefix: Option<&str>,
    dest_dir: &str,
    azure_client: &AzureClient,
) -> Result<()> {
    let blobs = azure_client.list_blobs(container, prefix).await?;

    if blobs.is_empty() {
        println!("{} No files found to download", "!".yellow());
        return Ok(());
    }

    // Create destination directory
    fs::create_dir_all(dest_dir).await?;

    let blob_count = blobs.len();
    for blob in blobs {
        let dest_path = format!("{}/{}", dest_dir.trim_end_matches('/'), blob.name);

        // Create parent directory if needed
        if let Some(parent) = get_parent_dir(&dest_path) {
            fs::create_dir_all(parent).await?;
        }

        println!(
            "{} Downloading az://{}/{} to {}",
            "←".blue(),
            container.yellow(),
            blob.name.cyan(),
            dest_path.cyan()
        );

        azure_client
            .download_file(container, &blob.name, &dest_path)
            .await?;
    }

    println!("{} Downloaded {} files", "✓".green(), blob_count);
    Ok(())
}

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
                copy_directory(entry_str, &dest_path).await?;
            } else {
                copy_file(entry_str, &dest_path).await?;
            }
        }

        Ok(())
    })
}

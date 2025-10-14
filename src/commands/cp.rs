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
    let source_is_azure = is_azure_uri(source);
    let dest_is_azure = is_azure_uri(destination);

    match (source_is_azure, dest_is_azure) {
        (false, true) => {
            // Local to Azure
            let azure_client = AzureClient::new();
            azure_client.check_prerequisites().await?;
            upload_to_azure(source, destination, recursive, parallel, &azure_client).await
        }
        (true, false) => {
            // Azure to Local
            let azure_client = AzureClient::new();
            azure_client.check_prerequisites().await?;
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

    // Validate that we have a container specified
    if dest_container.is_empty() {
        return Err(anyhow!(
            "Invalid destination URI '{}'. You must specify both storage account and container: az://<account>/<container>/[path]",
            destination
        ));
    }

    // Create azure client with account if specified in URI
    let client = if let Some(account_name) = account.clone() {
        AzureClient::new().with_storage_account(&account_name)
    } else {
        azure_client.clone()
    };

    // Warn if using legacy format without account
    if account.is_none() && client.get_storage_account().is_none() {
        eprintln!(
            "{} Using legacy URI format '{}'. Consider using the full format 'az://<account>/<container>' for better clarity.",
            "⚠".yellow(),
            destination.yellow()
        );
    }

    if !path_exists(source) {
        return Err(anyhow!("Source path '{}' does not exist", source));
    }

    if is_directory(source) {
        if !recursive {
            return Err(anyhow!(
                "Source is a directory. Use -r flag for recursive copy"
            ));
        }

        // Upload directory using efficient batch upload
        let account_name = account
            .as_ref()
            .map(|s| s.as_str())
            .or_else(|| client.get_storage_account());

        let dest_display = if let Some(acct) = account_name {
            if let Some(path) = &dest_path {
                format!("az://{}/{}/{}", acct, dest_container, path)
            } else {
                format!("az://{}/{}/", acct, dest_container)
            }
        } else {
            if let Some(path) = &dest_path {
                format!("az://{}/{}", dest_container, path)
            } else {
                format!("az://{}/", dest_container)
            }
        };

        println!(
            "{} Uploading directory {} to {} (using parallel batch upload with {} connections)",
            "→".green(),
            source.cyan(),
            dest_display.cyan(),
            _parallel
        );

        client
            .upload_batch(source, &dest_container, dest_path.as_deref(), _parallel)
            .await?;

        println!("{} Batch upload completed", "✓".green());
        Ok(())
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

        // Build full destination path for display
        let account_name = account
            .as_ref()
            .map(|s| s.as_str())
            .or_else(|| client.get_storage_account());
        let dest_display = if let Some(acct) = account_name {
            format!("az://{}/{}/{}", acct, dest_container, blob_name)
        } else {
            format!("az://{}/{}", dest_container, blob_name)
        };

        println!(
            "{} Uploading {} to {}",
            "→".green(),
            source.cyan(),
            dest_display.cyan()
        );

        client
            .upload_file(source, &dest_container, &blob_name)
            .await?;
        println!("{} Upload completed", "✓".green());
        Ok(())
    }
}

async fn download_from_azure(
    source: &str,
    destination: &str,
    recursive: bool,
    _parallel: u32,
    azure_client: &AzureClient,
) -> Result<()> {
    let (account, source_container, source_path) = parse_azure_uri(source)?;

    // Validate that we have a container specified
    if source_container.is_empty() {
        return Err(anyhow!(
            "Invalid source URI '{}'. You must specify both storage account and container: az://<account>/<container>/[path]",
            source
        ));
    }

    // Create azure client with account if specified in URI
    let client = if let Some(account_name) = account.clone() {
        AzureClient::new().with_storage_account(&account_name)
    } else {
        azure_client.clone()
    };

    // Warn if using legacy format without account
    if account.is_none() && client.get_storage_account().is_none() {
        eprintln!(
            "{} Using legacy URI format '{}'. Consider using the full format 'az://<account>/<container>' for better clarity.",
            "⚠".yellow(),
            source.yellow()
        );
    }

    if let Some(path) = source_path {
        if path.ends_with('/') || recursive {
            // Download directory/prefix
            let account_name = account
                .as_ref()
                .map(|s| s.as_str())
                .or_else(|| client.get_storage_account());
            download_directory(
                &source_container,
                Some(&path),
                destination,
                account_name,
                &client,
            )
            .await
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

            let account_name = account
                .as_ref()
                .map(|s| s.as_str())
                .or_else(|| client.get_storage_account());
            let source_display = if let Some(acct) = account_name {
                format!("az://{}/{}/{}", acct, source_container, path)
            } else {
                format!("az://{}/{}", source_container, path)
            };

            println!(
                "{} Downloading {} to {}",
                "←".blue(),
                source_display.cyan(),
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
        let account_name = account
            .as_ref()
            .map(|s| s.as_str())
            .or_else(|| client.get_storage_account());
        download_directory(&source_container, None, destination, account_name, &client).await
    }
}

async fn download_directory(
    container: &str,
    prefix: Option<&str>,
    dest_dir: &str,
    account_name: Option<&str>,
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

        let source_display = if let Some(acct) = account_name {
            format!("az://{}/{}/{}", acct, container, blob.name)
        } else {
            format!("az://{}/{}", container, blob.name)
        };

        println!(
            "{} Downloading {} to {}",
            "←".blue(),
            source_display.cyan(),
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

#[cfg(test)]
mod tests {
    #[test]
    fn test_local_to_azure_single_file_docs() {
        // This is a documentation test showing the expected behavior
        // In a real scenario, we would mock the AzureClient
        // Test case: azst cp /local/file.txt az://account/container/
        // Expected: Upload file.txt to container root
    }

    #[test]
    fn test_azure_to_local_single_file_docs() {
        // Test case: azst cp az://account/container/file.txt /local/
        // Expected: Download file.txt to /local/file.txt
    }

    #[test]
    fn test_local_to_local_copy_docs() {
        // Test case: azst cp /source/file.txt /dest/
        // Expected: Copy file.txt using standard filesystem operations
    }

    #[test]
    fn test_recursive_directory_upload_docs() {
        // Test case: azst cp -r /local/dir/ az://account/container/prefix/
        // Expected: Upload all files in directory with prefix
    }

    #[test]
    fn test_azure_to_azure_error_docs() {
        // Test case: azst cp az://account1/c1/file az://account2/c2/file
        // Expected: Error - not implemented
    }

    // Note: Full integration tests would require mocking Azure CLI calls
    // For now, these serve as documentation of expected behavior
}

use anyhow::{anyhow, Result};
use colored::*;

use crate::azure::AzureClient;
use crate::utils::{format_size, is_azure_uri, parse_azure_uri};

pub async fn execute(
    path: Option<&str>,
    long: bool,
    human_readable: bool,
    recursive: bool,
    account: Option<&str>,
) -> Result<()> {
    let mut azure_client = AzureClient::new();

    if let Some(account_name) = account {
        azure_client = azure_client.with_storage_account(account_name);
    }

    azure_client.check_prerequisites().await?;

    match path {
        Some(p) if is_azure_uri(p) => {
            list_azure_objects(p, long, human_readable, recursive, &azure_client).await
        }
        Some(p) => list_local_path(p, long, human_readable, recursive).await,
        None => {
            // List all containers
            list_containers(long, &azure_client).await
        }
    }
}

async fn list_containers(long: bool, azure_client: &AzureClient) -> Result<()> {
    let containers = azure_client.list_containers().await?;

    if containers.is_empty() {
        println!("No containers found");
        return Ok(());
    }

    println!("{}", "Azure Storage Containers:".bold());

    for container in containers {
        if long {
            println!(
                "{:<30} {}",
                format!("az://{}/", container.name).cyan(),
                container.properties.last_modified.dimmed()
            );
        } else {
            println!("{}", format!("az://{}/", container.name).cyan());
        }
    }

    Ok(())
}

async fn list_azure_objects(
    path: &str,
    long: bool,
    human_readable: bool,
    recursive: bool,
    azure_client: &AzureClient,
) -> Result<()> {
    let (account, container, prefix) = parse_azure_uri(path)?;

    // Create azure client with account if specified in URI
    let client = if let Some(account_name) = account {
        AzureClient::new().with_storage_account(&account_name)
    } else {
        azure_client.clone()
    };

    // If there's no prefix (just az://account/container or az://account/container/), list container contents
    let prefix = if recursive {
        prefix
    } else {
        // For non-recursive listing, we need to handle directory-like behavior
        prefix
    };

    let blobs = client.list_blobs(&container, prefix.as_deref()).await?;

    if blobs.is_empty() {
        println!("No objects found in az://{}/", container);
        return Ok(());
    }

    println!("{}", format!("Contents of az://{}/:", container).bold());

    if long {
        println!(
            "{:<10} {:<15} {:<20} {}",
            "Size".bold(),
            "Type".bold(),
            "Modified".bold(),
            "Name".bold()
        );
        println!("{}", "-".repeat(80).dimmed());
    }

    for blob in blobs {
        let size_str = if human_readable {
            format_size(blob.properties.content_length)
        } else {
            blob.properties.content_length.to_string()
        };

        let content_type = blob
            .properties
            .content_type
            .unwrap_or_else(|| "unknown".to_string());

        if long {
            println!(
                "{:<10} {:<15} {:<20} {}",
                size_str.green(),
                content_type.yellow(),
                blob.properties.last_modified.dimmed(),
                format!("az://{}/{}", container, blob.name).cyan()
            );
        } else {
            println!("{}", format!("az://{}/{}", container, blob.name).cyan());
        }
    }

    Ok(())
}

async fn list_local_path(
    path: &str,
    long: bool,
    human_readable: bool,
    recursive: bool,
) -> Result<()> {
    use std::path::Path;

    let path_obj = Path::new(path);

    if !path_obj.exists() {
        return Err(anyhow!("Path '{}' does not exist", path));
    }

    if path_obj.is_file() {
        // List single file
        list_single_file(path, long, human_readable).await
    } else if path_obj.is_dir() {
        // List directory contents
        list_directory(path, long, human_readable, recursive).await
    } else {
        Err(anyhow!("Path '{}' is neither file nor directory", path))
    }
}

async fn list_single_file(path: &str, long: bool, human_readable: bool) -> Result<()> {
    use tokio::fs;

    if long {
        let metadata = fs::metadata(path).await?;
        let size = metadata.len();
        let size_str = if human_readable {
            format_size(size)
        } else {
            size.to_string()
        };

        println!("{:<10} {}", size_str.green(), path.cyan());
    } else {
        println!("{}", path.cyan());
    }

    Ok(())
}

async fn list_directory(
    dir_path: &str,
    long: bool,
    human_readable: bool,
    recursive: bool,
) -> Result<()> {
    use tokio::fs;

    if long {
        println!(
            "{:<10} {:<10} {}",
            "Size".bold(),
            "Type".bold(),
            "Name".bold()
        );
        println!("{}", "-".repeat(50).dimmed());
    }

    if recursive {
        list_directory_recursive(dir_path, "", long, human_readable).await
    } else {
        let mut entries = fs::read_dir(dir_path).await?;

        while let Some(entry) = entries.next_entry().await? {
            let entry_path = entry.path();
            let entry_name = entry.file_name();
            let name_str = entry_name.to_str().unwrap_or("?");

            if long {
                let metadata = entry.metadata().await?;
                let size = metadata.len();
                let size_str = if human_readable {
                    format_size(size)
                } else {
                    size.to_string()
                };

                let type_str = if metadata.is_dir() {
                    "dir".to_string()
                } else {
                    "file".to_string()
                };

                let display_name = if metadata.is_dir() {
                    format!("{}/", name_str).blue()
                } else {
                    name_str.normal()
                };

                println!(
                    "{:<10} {:<10} {}",
                    size_str.green(),
                    type_str.yellow(),
                    display_name
                );
            } else {
                let display_name = if entry_path.is_dir() {
                    format!("{}/", name_str).blue()
                } else {
                    name_str.to_string().normal()
                };
                println!("{}", display_name);
            }
        }

        Ok(())
    }
}

fn list_directory_recursive<'a>(
    dir_path: &'a str,
    prefix: &'a str,
    long: bool,
    human_readable: bool,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
    Box::pin(async move {
        use tokio::fs;

        let mut entries = fs::read_dir(dir_path).await?;

        while let Some(entry) = entries.next_entry().await? {
            let entry_path = entry.path();
            let entry_name = entry.file_name();
            let name_str = entry_name.to_str().unwrap_or("?");
            let full_name = if prefix.is_empty() {
                name_str.to_string()
            } else {
                format!("{}/{}", prefix, name_str)
            };

            if long {
                let metadata = entry.metadata().await?;
                let size = metadata.len();
                let size_str = if human_readable {
                    format_size(size)
                } else {
                    size.to_string()
                };

                let type_str = if metadata.is_dir() {
                    "dir".to_string()
                } else {
                    "file".to_string()
                };

                let display_name = if metadata.is_dir() {
                    format!("{}/", full_name).blue()
                } else {
                    full_name.normal()
                };

                println!(
                    "{:<10} {:<10} {}",
                    size_str.green(),
                    type_str.yellow(),
                    display_name
                );
            } else {
                let display_name = if entry_path.is_dir() {
                    format!("{}/", full_name).blue()
                } else {
                    full_name.normal()
                };
                println!("{}", display_name);
            }

            // Recursively list subdirectories
            if entry_path.is_dir() {
                let entry_str = entry_path.to_str().unwrap();
                list_directory_recursive(entry_str, &full_name, long, human_readable).await?;
            }
        }

        Ok(())
    })
}

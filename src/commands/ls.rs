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
    match path {
        Some(p) if is_azure_uri(p) => {
            let mut azure_client = AzureClient::new();
            if let Some(account_name) = account {
                azure_client = azure_client.with_storage_account(account_name);
            }
            azure_client.check_prerequisites().await?;
            list_azure_objects(p, long, human_readable, recursive, &azure_client).await
        }
        Some(p) => list_local_path(p, long, human_readable, recursive).await,
        None => {
            // List all storage accounts - requires Azure
            let azure_client = AzureClient::new();
            azure_client.check_prerequisites().await?;
            list_storage_accounts(long, &azure_client).await
        }
    }
}

async fn list_storage_accounts(long: bool, azure_client: &AzureClient) -> Result<()> {
    let accounts = azure_client.list_storage_accounts().await?;

    if accounts.is_empty() {
        println!("No storage accounts found");
        return Ok(());
    }

    println!("{}", "Azure Storage Accounts:".bold());

    for account in accounts {
        if long {
            println!(
                "{:<30} {:<15} {}",
                format!("az://{}/", account.name).cyan(),
                account.location.dimmed(),
                account.resource_group.yellow()
            );
        } else {
            println!("{}", format!("az://{}/", account.name).cyan());
        }
    }

    Ok(())
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
    let client = if let Some(account_name) = account.clone() {
        AzureClient::new().with_storage_account(&account_name)
    } else {
        azure_client.clone()
    };

    // Special case: If we have an account but no container (az://account or az://account/),
    // list all containers in that account
    if account.is_some() && container.is_empty() {
        return list_containers(long, &client).await;
    }

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

#[cfg(test)]
mod tests {
    #[test]
    fn test_list_containers_docs() {
        // Test case: azst ls
        // Expected: List all containers in default storage account
    }

    #[test]
    fn test_list_container_contents_docs() {
        // Test case: azst ls az://account/container/
        // Expected: List all blobs in container
    }

    #[test]
    fn test_list_with_prefix_docs() {
        // Test case: azst ls az://account/container/prefix/
        // Expected: List blobs starting with prefix
    }

    #[test]
    fn test_list_long_format_docs() {
        // Test case: azst ls -l az://account/container/
        // Expected: Display size, type, modified date, and name
    }

    #[test]
    fn test_list_human_readable_docs() {
        // Test case: azst ls -lh az://account/container/
        // Expected: Display sizes in KB/MB/GB format
    }

    #[test]
    fn test_list_recursive_docs() {
        // Test case: azst ls -r az://account/container/
        // Expected: List all blobs recursively (Azure lists all by default)
    }

    #[test]
    fn test_list_local_file_docs() {
        // Test case: azst ls /local/file.txt
        // Expected: Display file info
    }

    #[test]
    fn test_list_local_directory_docs() {
        // Test case: azst ls /local/dir/
        // Expected: List directory contents
    }

    // Note: Full integration tests would require mocking Azure CLI calls
}

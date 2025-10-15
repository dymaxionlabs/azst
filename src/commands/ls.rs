use anyhow::{anyhow, Result};
use colored::*;
use std::io::{self, IsTerminal};

use crate::azure::{AzureClient, BlobItem};
use crate::utils::{
    contains_recursive_wildcard, format_size, is_azure_uri, matches_pattern, parse_azure_uri,
    split_wildcard_path,
};

/// Calculate the depth of a pattern (number of path segments)
/// Treats ** as matching any depth
fn pattern_depth(pattern: &str) -> Option<usize> {
    if pattern.contains("**") {
        None // Infinite depth
    } else {
        // Count the number of path segments
        Some(pattern.split('/').filter(|s| !s.is_empty()).count())
    }
}

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

    let is_tty = io::stdout().is_terminal();

    if is_tty {
        println!("{}", "Azure Storage Accounts:".bold());
    }

    for account in accounts {
        if long {
            if is_tty {
                println!(
                    "{:<30} {:<15} {}",
                    format!("az://{}/", account.name).cyan(),
                    account.location.dimmed(),
                    account.resource_group.yellow()
                );
            } else {
                println!(
                    "{:<30} {:<15} {}",
                    format!("az://{}/", account.name),
                    account.location,
                    account.resource_group
                );
            }
        } else {
            if is_tty {
                println!("{}", format!("az://{}/", account.name).cyan());
            } else {
                println!("{}", format!("az://{}/", account.name));
            }
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

    let is_tty = io::stdout().is_terminal();

    if is_tty {
        println!("{}", "Azure Storage Containers:".bold());
    }

    // Get the account name from the client
    let account_name = azure_client
        .get_storage_account()
        .ok_or_else(|| anyhow!("Storage account not configured"))?;

    for container in containers {
        if long {
            if is_tty {
                println!(
                    "{:<30} {}",
                    format!("az://{}/{}/", account_name, container.name).cyan(),
                    container.properties.last_modified.dimmed()
                );
            } else {
                println!(
                    "{:<30} {}",
                    format!("az://{}/{}/", account_name, container.name),
                    container.properties.last_modified
                );
            }
        } else {
            if is_tty {
                println!(
                    "{}",
                    format!("az://{}/{}/", account_name, container.name).cyan()
                );
            } else {
                println!("{}", format!("az://{}/{}/", account_name, container.name));
            }
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

    //Check if the prefix contains wildcards
    let (list_prefix, pattern, force_recursive) = if let Some(prefix_str) = &prefix {
        if let Some((before_wildcard, mut wildcard_pattern)) = split_wildcard_path(prefix_str) {
            // Has wildcard: list with prefix before wildcard, then filter with pattern
            // If pattern contains **, force recursive listing (no delimiter)
            // Also force recursive if the wildcard pattern contains / (multi-segment pattern)
            let is_recursive =
                contains_recursive_wildcard(&wildcard_pattern) || wildcard_pattern.contains('/');

            // If pattern ends with /, append * to match contents of that directory
            if wildcard_pattern.ends_with('/') {
                wildcard_pattern.push('*');
            }

            (
                if before_wildcard.is_empty() {
                    None
                } else {
                    Some(before_wildcard)
                },
                Some(wildcard_pattern),
                is_recursive,
            )
        } else {
            // No wildcard: use prefix as-is
            (prefix.clone(), None, false)
        }
    } else {
        // No prefix at all
        (None, None, false)
    };

    // Use delimiter for non-recursive listing (hierarchical, like gsutil default behavior)
    // Omit delimiter for recursive listing or when using ** wildcard
    let delimiter = if recursive || force_recursive {
        None
    } else {
        Some("/")
    };

    // Get the actual account name being used
    let actual_account = client
        .get_storage_account()
        .ok_or_else(|| anyhow!("Storage account not configured"))?;

    let blobs = client
        .list_blobs(&container, list_prefix.as_deref(), delimiter)
        .await?;

    // Filter blobs if we have a pattern
    let filtered_blobs: Vec<BlobItem> = if let Some(ref pattern_str) = pattern {
        // Calculate the expected depth based on the pattern
        let expected_depth = pattern_depth(pattern_str);

        // If we have a specific depth (not **) and we're NOT in explicit recursive mode,
        // we need to extract directory prefixes at that depth (hierarchical view)
        if let Some(depth) = expected_depth {
            if force_recursive && !recursive {
                // Extract unique prefixes at the target depth (non-recursive mode with multi-segment pattern)
                let mut unique_prefixes = std::collections::HashSet::new();

                for item in &blobs {
                    let name = match item {
                        BlobItem::Blob(blob) => &blob.name,
                        BlobItem::Prefix(prefix) => prefix,
                    };

                    let match_part = if let Some(ref prefix) = list_prefix {
                        name.strip_prefix(prefix).unwrap_or(name)
                    } else {
                        name
                    };

                    // Extract prefix at target depth
                    let segments: Vec<&str> = match_part.split('/').collect();
                    if segments.len() >= depth {
                        let prefix_at_depth = segments[..depth].join("/") + "/";

                        // Check if this prefix matches the pattern
                        if matches_pattern(&prefix_at_depth, pattern_str) {
                            unique_prefixes.insert(prefix_at_depth);
                        }
                    }
                }

                // Convert prefixes to BlobItem::Prefix
                unique_prefixes
                    .into_iter()
                    .map(|prefix| {
                        let full_name = if let Some(ref list_pfx) = list_prefix {
                            format!("{}{}", list_pfx, prefix)
                        } else {
                            prefix
                        };
                        BlobItem::Prefix(full_name)
                    })
                    .collect()
            } else {
                // Regular filtering for non-recursive with delimiter
                blobs
                    .into_iter()
                    .filter(|item| {
                        let name = match item {
                            BlobItem::Blob(blob) => &blob.name,
                            BlobItem::Prefix(prefix) => prefix,
                        };

                        let match_part = if let Some(ref prefix) = list_prefix {
                            name.strip_prefix(prefix).unwrap_or(name)
                        } else {
                            name
                        };

                        matches_pattern(match_part, pattern_str)
                    })
                    .collect()
            }
        } else {
            // ** pattern - show all matches at any depth
            blobs
                .into_iter()
                .filter(|item| {
                    let name = match item {
                        BlobItem::Blob(blob) => &blob.name,
                        BlobItem::Prefix(prefix) => prefix,
                    };

                    let match_part = if let Some(ref prefix) = list_prefix {
                        name.strip_prefix(prefix).unwrap_or(name)
                    } else {
                        name
                    };

                    matches_pattern(match_part, pattern_str)
                })
                .collect()
        }
    } else {
        blobs
    };

    if filtered_blobs.is_empty() {
        if pattern.is_some() {
            println!(
                "No objects matching pattern in az://{}/{}/",
                actual_account, container
            );
        } else {
            println!("No objects found in az://{}/{}/", actual_account, container);
        }
        return Ok(());
    }

    // Check if stdout is a TTY - only show headers/colors when outputting to terminal
    let is_tty = io::stdout().is_terminal();

    if is_tty {
        println!(
            "{}",
            format!("Contents of az://{}/{}:", actual_account, container).bold()
        );
    }

    if long && is_tty {
        println!(
            "{:<10} {:<15} {:<20} {}",
            "Size".bold(),
            "Type".bold(),
            "Modified".bold(),
            "Name".bold()
        );
        println!("{}", "-".repeat(80).dimmed());
    }

    for item in filtered_blobs {
        match item {
            BlobItem::Blob(blob) => {
                let size_str = if human_readable {
                    format_size(blob.properties.content_length)
                } else {
                    blob.properties.content_length.to_string()
                };

                let content_type = blob
                    .properties
                    .content_type
                    .unwrap_or_else(|| "unknown".to_string());

                let blob_uri = format!("az://{}/{}/{}", actual_account, container, blob.name);

                if long {
                    if is_tty {
                        println!(
                            "{:<10} {:<15} {:<20} {}",
                            size_str.green(),
                            content_type.yellow(),
                            blob.properties.last_modified.dimmed(),
                            blob_uri.cyan()
                        );
                    } else {
                        println!(
                            "{:<10} {:<15} {:<20} {}",
                            size_str, content_type, blob.properties.last_modified, blob_uri
                        );
                    }
                } else {
                    if is_tty {
                        println!("{}", blob_uri.cyan());
                    } else {
                        println!("{}", blob_uri);
                    }
                }
            }
            BlobItem::Prefix(prefix) => {
                // Display directory/prefix with trailing slash
                let prefix_uri = format!("az://{}/{}/{}", actual_account, container, prefix);

                if long {
                    if is_tty {
                        println!(
                            "{:<10} {:<15} {:<20} {}",
                            "-".dimmed(),
                            "DIR".blue(),
                            "-".dimmed(),
                            prefix_uri.blue().bold()
                        );
                    } else {
                        println!("{:<10} {:<15} {:<20} {}", "-", "DIR", "-", prefix_uri);
                    }
                } else {
                    if is_tty {
                        println!("{}", prefix_uri.blue().bold());
                    } else {
                        println!("{}", prefix_uri);
                    }
                }
            }
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

    let is_tty = io::stdout().is_terminal();

    if long {
        let metadata = fs::metadata(path).await?;
        let size = metadata.len();
        let size_str = if human_readable {
            format_size(size)
        } else {
            size.to_string()
        };

        if is_tty {
            println!("{:<10} {}", size_str.green(), path.cyan());
        } else {
            println!("{:<10} {}", size_str, path);
        }
    } else {
        if is_tty {
            println!("{}", path.cyan());
        } else {
            println!("{}", path);
        }
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

    let is_tty = io::stdout().is_terminal();

    if long && is_tty {
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

                if is_tty {
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
                    let display_name = if metadata.is_dir() {
                        format!("{}/", name_str)
                    } else {
                        name_str.to_string()
                    };

                    println!("{:<10} {:<10} {}", size_str, type_str, display_name);
                }
            } else {
                if is_tty {
                    let display_name = if entry_path.is_dir() {
                        format!("{}/", name_str).blue()
                    } else {
                        name_str.to_string().normal()
                    };
                    println!("{}", display_name);
                } else {
                    let display_name = if entry_path.is_dir() {
                        format!("{}/", name_str)
                    } else {
                        name_str.to_string()
                    };
                    println!("{}", display_name);
                }
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

        let is_tty = io::stdout().is_terminal();
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

                if is_tty {
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
                    let display_name = if metadata.is_dir() {
                        format!("{}/", full_name)
                    } else {
                        full_name.to_string()
                    };

                    println!("{:<10} {:<10} {}", size_str, type_str, display_name);
                }
            } else {
                if is_tty {
                    let display_name = if entry_path.is_dir() {
                        format!("{}/", full_name).blue()
                    } else {
                        full_name.normal()
                    };
                    println!("{}", display_name);
                } else {
                    let display_name = if entry_path.is_dir() {
                        format!("{}/", full_name)
                    } else {
                        full_name.to_string()
                    };
                    println!("{}", display_name);
                }
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

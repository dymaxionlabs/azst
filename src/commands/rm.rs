use anyhow::{anyhow, Result};
use colored::*;
use std::io::{self, Write};

use crate::azure::AzureClient;
use crate::utils::{is_azure_uri, parse_azure_uri};

pub async fn execute(path: &str, recursive: bool, force: bool) -> Result<()> {
    let azure_client = AzureClient::new();
    azure_client.check_prerequisites().await?;

    if is_azure_uri(path) {
        remove_azure_object(path, recursive, force, &azure_client).await
    } else {
        remove_local_path(path, recursive, force).await
    }
}

async fn remove_azure_object(
    path: &str,
    recursive: bool,
    force: bool,
    azure_client: &AzureClient,
) -> Result<()> {
    let (account, container, blob_path) = parse_azure_uri(path)?;

    // Create azure client with account if specified in URI
    let client = if let Some(account_name) = account {
        AzureClient::new().with_storage_account(&account_name)
    } else {
        azure_client.clone()
    };

    if let Some(path) = blob_path {
        if path.ends_with('/') || recursive {
            // Remove multiple objects with prefix
            remove_azure_prefix(&container, Some(&path), recursive, force, &client).await
        } else {
            // Remove single blob
            remove_single_blob(&container, &path, force, &client).await
        }
    } else {
        return Err(anyhow!(
            "Cannot remove entire container with rm. Use 'azst rb' instead"
        ));
    }
}

async fn remove_single_blob(
    container: &str,
    blob_name: &str,
    force: bool,
    azure_client: &AzureClient,
) -> Result<()> {
    if !force {
        print!(
            "Remove az://{}/{}? (y/N): ",
            container.yellow(),
            blob_name.cyan()
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

    println!(
        "{} Removing az://{}/{}",
        "×".red(),
        container.yellow(),
        blob_name.cyan()
    );

    azure_client.delete_blob(container, blob_name).await?;
    println!("{} Removed", "✓".green());

    Ok(())
}

async fn remove_azure_prefix(
    container: &str,
    prefix: Option<&str>,
    recursive: bool,
    force: bool,
    azure_client: &AzureClient,
) -> Result<()> {
    if !recursive {
        return Err(anyhow!("Removing multiple objects requires -r flag"));
    }

    let blobs = azure_client.list_blobs(container, prefix).await?;

    if blobs.is_empty() {
        println!("No objects found to remove");
        return Ok(());
    }

    if !force {
        println!("Found {} objects to remove:", blobs.len());
        for (i, blob) in blobs.iter().enumerate() {
            if i < 5 {
                println!("  az://{}/{}", container.yellow(), blob.name.cyan());
            } else if i == 5 {
                println!("  ... and {} more", blobs.len() - 5);
                break;
            }
        }

        print!("Remove all {} objects? (y/N): ", blobs.len());
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim().to_lowercase();

        if input != "y" && input != "yes" {
            println!("Aborted");
            return Ok(());
        }
    }

    let mut removed_count = 0;
    for blob in blobs {
        println!(
            "{} Removing az://{}/{}",
            "×".red(),
            container.yellow(),
            blob.name.cyan()
        );

        azure_client.delete_blob(container, &blob.name).await?;
        removed_count += 1;
    }

    println!("{} Removed {} objects", "✓".green(), removed_count);
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

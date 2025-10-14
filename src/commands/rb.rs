use anyhow::{anyhow, Result};
use colored::*;
use std::io::{self, Write};

use crate::azure::AzureClient;
use crate::utils::{is_azure_uri, parse_azure_uri};

pub async fn execute(container_uri: &str, force: bool) -> Result<()> {
    if !is_azure_uri(container_uri) {
        return Err(anyhow!(
            "Container URI must be in format 'az://account/container' or 'az://container'"
        ));
    }

    let (account, container_name, path) = parse_azure_uri(container_uri)?;

    if path.is_some() {
        return Err(anyhow!(
            "Cannot specify path when removing container. Use 'az://account/container' or 'az://container' format"
        ));
    }

    let azure_client = if let Some(account_name) = account {
        AzureClient::new().with_storage_account(&account_name)
    } else {
        AzureClient::new()
    };

    azure_client.check_prerequisites().await?;

    // Check if container has contents
    let blobs = azure_client.list_blobs(&container_name, None).await?;
    let has_contents = !blobs.is_empty();

    if has_contents && !force {
        println!(
            "{} Container '{}' contains {} objects",
            "!".yellow(),
            container_name.yellow(),
            blobs.len()
        );

        print!("Remove container and all its contents? (y/N): ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim().to_lowercase();

        if input != "y" && input != "yes" {
            println!("Aborted");
            return Ok(());
        }
    } else if !has_contents && !force {
        print!(
            "Remove empty container '{}'? (y/N): ",
            container_name.yellow()
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
        "{} Removing container: {}",
        "×".red(),
        format!("az://{}", container_name).cyan()
    );

    azure_client.delete_container(&container_name).await?;

    println!(
        "{} Container '{}' removed successfully",
        "✓".green(),
        container_name.yellow()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_remove_empty_container_docs() {
        // Test case: azst rb az://account/container
        // Expected: Remove empty container after confirmation
    }

    #[test]
    fn test_remove_non_empty_container_force_docs() {
        // Test case: azst rb -f az://account/container
        // Expected: Remove container with contents without confirmation
    }

    #[test]
    fn test_remove_container_invalid_uri_docs() {
        // Test case: azst rb /local/path
        // Expected: Error - must be Azure URI
    }

    #[test]
    fn test_remove_container_with_path_error_docs() {
        // Test case: azst rb az://account/container/path
        // Expected: Error - cannot specify path
    }

    #[test]
    fn test_remove_container_with_confirmation_docs() {
        // Test case: azst rb az://account/container (user input: n)
        // Expected: Abort removal
    }
}

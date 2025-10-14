use anyhow::{anyhow, Result};
use colored::*;

use crate::azure::AzureClient;
use crate::utils::{is_azure_uri, parse_azure_uri};

pub async fn execute(container_uri: &str, account: Option<&str>) -> Result<()> {
    if !is_azure_uri(container_uri) {
        return Err(anyhow!(
            "Container URI must be in format 'az://account/container' or 'az://container'"
        ));
    }

    let (uri_account, container_name, path) = parse_azure_uri(container_uri)?;

    if path.is_some() {
        return Err(anyhow!(
            "Cannot specify path when creating container. Use 'az://account/container' or 'az://container' format"
        ));
    }

    // Use account from URI if provided, otherwise use --account flag
    let account_to_use = uri_account.or_else(|| account.map(|s| s.to_string()));

    let mut azure_client = AzureClient::new();

    if let Some(account_name) = account_to_use {
        azure_client = azure_client.with_storage_account(&account_name);
    }

    azure_client.check_prerequisites().await?;

    println!(
        "{} Creating container: {}",
        "→".green(),
        format!("az://{}", container_name).cyan()
    );

    azure_client.create_container(&container_name).await?;

    println!(
        "{} Container '{}' created successfully",
        "✓".green(),
        container_name.yellow()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_create_container_with_account_in_uri_docs() {
        // Test case: azst mb az://account/container
        // Expected: Create container in specified account
    }

    #[test]
    fn test_create_container_with_account_flag_docs() {
        // Test case: azst mb --account myaccount az://container
        // Expected: Create container using --account flag
    }

    #[test]
    fn test_create_container_invalid_uri_docs() {
        // Test case: azst mb /local/path
        // Expected: Error - must be Azure URI
    }

    #[test]
    fn test_create_container_with_path_error_docs() {
        // Test case: azst mb az://account/container/path
        // Expected: Error - cannot specify path
    }
}

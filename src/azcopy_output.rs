use anyhow::Result;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct AzCopyLogEntry {
    pub time_stamp: String,
    pub message_type: String,
    pub message_content: String,
    #[serde(default)]
    pub prompt_details: Option<Value>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct ProgressMessage {
    pub error_msg: String,
    #[serde(rename = "JobID")]
    pub job_id: String,
    pub active_connections: String,
    pub complete_job_ordered: bool,
    pub job_status: String,
    pub total_transfers: String,
    pub file_transfers: String,
    pub transfers_completed: String,
    pub transfers_failed: String,
    pub transfers_skipped: String,
    pub bytes_over_wire: String,
    pub total_bytes_transferred: String,
    pub total_bytes_expected: String,
    pub percent_complete: String,
    #[serde(rename = "AverageIOPS")]
    pub average_iops: String,
    #[serde(rename = "AverageE2EMilliseconds")]
    pub average_e2e_milliseconds: String,
    pub server_busy_percentage: String,
    pub network_error_percentage: String,
    // Additional fields that may be present
    #[serde(default)]
    pub failed_transfers: Option<Value>,
    #[serde(default)]
    pub skipped_transfers: Option<Value>,
    #[serde(default)]
    pub perf_constraint: Option<i32>,
    #[serde(default)]
    pub performance_advice: Option<Value>,
    #[serde(default)]
    pub is_cleanup_job: Option<bool>,
    #[serde(default)]
    pub skipped_symlink_count: Option<String>,
    #[serde(default)]
    pub hardlinks_converted_count: Option<String>,
    #[serde(default)]
    pub skipped_special_file_count: Option<String>,
    #[serde(default)]
    pub folders_completed: Option<String>,
    #[serde(default)]
    pub folder_property_transfers: Option<String>,
    #[serde(default)]
    pub symlink_transfers: Option<String>,
    #[serde(default)]
    pub folders_failed: Option<String>,
    #[serde(default)]
    pub folders_skipped: Option<String>,
    #[serde(default)]
    pub total_bytes_enumerated: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct InitMessage {
    pub log_file_location: String,
    #[serde(rename = "JobID")]
    pub job_id: String,
    pub is_cleanup_job: bool,
}

/// Parse and display AzCopy JSON output with a progress bar
/// Returns the number of failed transfers
pub async fn handle_azcopy_output<R: AsyncRead + Unpin>(stream: R) -> Result<u32> {
    let reader = BufReader::new(stream);
    let mut lines = reader.lines();
    let mut pb: Option<ProgressBar> = None;
    let mut failed_count: u32 = 0;
    let mut log_file_location: Option<String> = None;

    while let Some(line) = lines.next_line().await? {
        // Try to parse as JSON log entry first
        if let Ok(entry) = serde_json::from_str::<AzCopyLogEntry>(&line) {
            match entry.message_type.as_str() {
                "Info" => {
                    // Print info messages, stripping "INFO: " prefix
                    let msg = entry.message_content.trim();
                    let msg = msg.strip_prefix("INFO: ").unwrap_or(msg);
                    println!("{} {}", "ℹ".blue(), msg);
                }
                "Progress" => {
                    // Parse the nested JSON in MessageContent
                    match serde_json::from_str::<ProgressMessage>(&entry.message_content) {
                        Ok(progress) => {
                            // Check if job is completed or completed with errors
                            if progress.job_status == "Completed"
                                || progress.job_status == "CompletedWithErrors"
                            {
                                if let Some(ref progress_bar) = pb {
                                    progress_bar.finish_and_clear();
                                    pb = None;
                                }

                                // Print completion summary
                                let completed = &progress.transfers_completed;
                                let total = &progress.total_transfers;
                                let bytes_transferred =
                                    format_bytes(&progress.total_bytes_transferred);
                                let failed = &progress.transfers_failed;

                                // Track failed count
                                failed_count = failed.parse::<u32>().unwrap_or(0);

                                if failed_count > 0 {
                                    println!(
                                        "{} {} of {} files transferred ({}) - {} failed",
                                        "⚠".yellow(),
                                        completed,
                                        total,
                                        bytes_transferred,
                                        failed
                                    );
                                    if let Some(ref log_path) = log_file_location {
                                        println!("{} Log file: {}", "ℹ".blue(), log_path.dimmed());
                                    }
                                } else {
                                    println!(
                                        "{} {} files transferred ({})",
                                        "✓".green(),
                                        completed,
                                        bytes_transferred
                                    );
                                }
                                continue;
                            }

                            // Create progress bar on first progress message
                            if pb.is_none() {
                                let progress_bar = ProgressBar::new(100);
                                progress_bar.set_style(
                                ProgressStyle::default_bar()
                                    .template(
                                        "{spinner:.green} [{bar:40.cyan/blue}] {percent}% {msg}",
                                    )
                                    .expect("Invalid progress bar template")
                                    .progress_chars("#>-"),
                            );
                                pb = Some(progress_bar);
                            }

                            // Update progress bar
                            if let Some(ref progress_bar) = pb {
                                let percent: f64 = progress.percent_complete.parse().unwrap_or(0.0);
                                progress_bar.set_position(percent as u64);

                                // Format additional info
                                let completed = &progress.transfers_completed;
                                let total = &progress.total_transfers;
                                let bytes_transferred =
                                    format_bytes(&progress.total_bytes_transferred);
                                let bytes_total = format_bytes(&progress.total_bytes_expected);

                                progress_bar.set_message(format!(
                                    "{}/{} files | {}/{} | {} IOPS",
                                    completed,
                                    total,
                                    bytes_transferred,
                                    bytes_total,
                                    progress.average_iops
                                ));
                            }
                        }
                        Err(_e) => {
                            // Failed to parse progress message, silently ignore
                        }
                    }
                }
                "Error" => {
                    // Print error messages
                    if let Some(ref progress_bar) = pb {
                        progress_bar.finish_and_clear();
                    }
                    eprintln!("{} {}", "✗".red().bold(), entry.message_content.red());
                }
                "Init" => {
                    // Job initialization - extract log file location
                    if let Ok(init) = serde_json::from_str::<InitMessage>(&entry.message_content) {
                        log_file_location = Some(init.log_file_location);
                    }
                }
                "EndOfJob" => {
                    // End of job message - parse to show final status
                    if let Ok(_progress) =
                        serde_json::from_str::<ProgressMessage>(&entry.message_content)
                    {
                        if let Some(ref progress_bar) = pb {
                            progress_bar.finish_and_clear();
                            pb = None;
                        }

                        // Already handled in Progress messages, but ensure bar is cleared
                    }
                }
                _ => {
                    // Unknown message type, print as-is
                    println!("{}", entry.message_content);
                }
            }
        } else if let Ok(progress) = serde_json::from_str::<ProgressMessage>(&line) {
            // Sometimes AzCopy prints raw ProgressMessage JSON without wrapper

            // Check if job is completed or completed with errors
            if progress.job_status == "Completed" || progress.job_status == "CompletedWithErrors" {
                if let Some(ref progress_bar) = pb {
                    progress_bar.finish_and_clear();
                    pb = None;
                }

                // Print completion summary
                let completed = &progress.transfers_completed;
                let total = &progress.total_transfers;
                let bytes_transferred = format_bytes(&progress.total_bytes_transferred);
                let failed = &progress.transfers_failed;

                // Track failed count
                failed_count = failed.parse::<u32>().unwrap_or(0);

                if failed_count > 0 {
                    println!(
                        "{} {} of {} files transferred ({}) - {} failed",
                        "⚠".yellow(),
                        completed,
                        total,
                        bytes_transferred,
                        failed
                    );
                    if let Some(ref log_path) = log_file_location {
                        println!("{} Log file: {}", "ℹ".blue(), log_path.dimmed());
                    }
                } else {
                    println!(
                        "{} {} files transferred ({})",
                        "✓".green(),
                        completed,
                        bytes_transferred
                    );
                }
                continue;
            }

            // Create progress bar on first progress message
            if pb.is_none() {
                let progress_bar = ProgressBar::new(100);
                progress_bar.set_style(
                    ProgressStyle::default_bar()
                        .template("{spinner:.green} [{bar:40.cyan/blue}] {percent}% {msg}")
                        .expect("Invalid progress bar template")
                        .progress_chars("#>-"),
                );
                pb = Some(progress_bar);
            }

            // Update progress bar
            if let Some(ref progress_bar) = pb {
                let percent: f64 = progress.percent_complete.parse().unwrap_or(0.0);
                progress_bar.set_position(percent as u64);

                // Format additional info
                let completed = &progress.transfers_completed;
                let total = &progress.total_transfers;
                let bytes_transferred = format_bytes(&progress.total_bytes_transferred);
                let bytes_total = format_bytes(&progress.total_bytes_expected);

                progress_bar.set_message(format!(
                    "{}/{} files | {}/{} | {} IOPS",
                    completed, total, bytes_transferred, bytes_total, progress.average_iops
                ));
            }
        }
    }

    // If progress bar still exists, finish it
    if let Some(ref progress_bar) = pb {
        progress_bar.finish_and_clear();
    }

    Ok(failed_count)
}

/// Format bytes into human-readable format
fn format_bytes(bytes_str: &str) -> String {
    if let Ok(bytes) = bytes_str.parse::<u64>() {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = bytes as f64;
        let mut unit_idx = 0;

        while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
            size /= 1024.0;
            unit_idx += 1;
        }

        format!("{:.2} {}", size, UNITS[unit_idx])
    } else {
        bytes_str.to_string()
    }
}

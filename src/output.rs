use colored::*;
use std::io::{self, IsTerminal};

/// Trait for output formatting strategies
/// Allows different output formats (TTY with colors, plain text, JSON, etc.)
pub trait OutputWriter: Send {
    /// Write a header/title
    fn write_header(&self, text: &str);

    /// Write column headers for long format
    fn write_table_header(&self, columns: &[(&str, usize)]);

    /// Write a separator line
    fn write_separator(&self, length: usize);

    /// Write a storage account entry
    fn write_storage_account(&self, name: &str, location: &str, resource_group: &str, long: bool);

    /// Write a container entry
    fn write_container(&self, account: &str, name: &str, modified: &str, long: bool);

    /// Write a blob entry
    fn write_blob(&self, uri: &str, size: &str, content_type: &str, modified: &str, long: bool);

    /// Write a prefix/directory entry
    fn write_prefix(&self, uri: &str, long: bool);

    /// Write a local file entry
    fn write_local_file(&self, name: &str, size: &str, file_type: &str, long: bool);
}

/// TTY writer with colors and formatting for human reading
pub struct TtyWriter;

impl OutputWriter for TtyWriter {
    fn write_header(&self, text: &str) {
        println!("{}", text.bold());
    }

    fn write_table_header(&self, columns: &[(&str, usize)]) {
        let formatted: Vec<String> = columns
            .iter()
            .map(|(name, width)| format!("{:<width$}", name.bold(), width = width))
            .collect();
        println!("{}", formatted.join(" "));
    }

    fn write_separator(&self, length: usize) {
        println!("{}", "-".repeat(length).dimmed());
    }

    fn write_storage_account(&self, name: &str, location: &str, resource_group: &str, long: bool) {
        let uri = format!("az://{}/", name).cyan();
        if long {
            println!(
                "{:<30} {:<15} {}",
                uri,
                location.dimmed(),
                resource_group.yellow()
            );
        } else {
            println!("{}", uri);
        }
    }

    fn write_container(&self, account: &str, name: &str, modified: &str, long: bool) {
        let uri = format!("az://{}/{}/", account, name).cyan();
        if long {
            println!("{:<30} {}", uri, modified.dimmed());
        } else {
            println!("{}", uri);
        }
    }

    fn write_blob(&self, uri: &str, size: &str, content_type: &str, modified: &str, long: bool) {
        if long {
            println!(
                "{:<10} {:<15} {:<20} {}",
                size.green(),
                content_type.yellow(),
                modified.dimmed(),
                uri.cyan()
            );
        } else {
            println!("{}", uri.cyan());
        }
    }

    fn write_prefix(&self, uri: &str, long: bool) {
        if long {
            println!(
                "{:<10} {:<15} {:<20} {}",
                "-".dimmed(),
                "DIR".blue(),
                "-".dimmed(),
                uri.blue().bold()
            );
        } else {
            println!("{}", uri.blue().bold());
        }
    }

    fn write_local_file(&self, name: &str, size: &str, file_type: &str, long: bool) {
        if long {
            let display_name = if file_type == "dir" {
                name.blue()
            } else {
                name.normal()
            };
            println!(
                "{:<10} {:<10} {}",
                size.green(),
                file_type.yellow(),
                display_name
            );
        } else {
            let display_name = if file_type == "dir" {
                name.blue()
            } else {
                name.normal()
            };
            println!("{}", display_name);
        }
    }
}

/// Plain text writer for piping/scripting (no colors)
pub struct PlainWriter;

impl OutputWriter for PlainWriter {
    fn write_header(&self, text: &str) {
        println!("{}", text);
    }

    fn write_table_header(&self, columns: &[(&str, usize)]) {
        let formatted: Vec<String> = columns
            .iter()
            .map(|(name, width)| format!("{:<width$}", name, width = width))
            .collect();
        println!("{}", formatted.join(" "));
    }

    fn write_separator(&self, _length: usize) {
        // No separator in plain output
    }

    fn write_storage_account(&self, name: &str, location: &str, resource_group: &str, long: bool) {
        let uri = format!("az://{}/", name);
        if long {
            println!("{:<30} {:<15} {}", uri, location, resource_group);
        } else {
            println!("{}", uri);
        }
    }

    fn write_container(&self, account: &str, name: &str, modified: &str, long: bool) {
        let uri = format!("az://{}/{}/", account, name);
        if long {
            println!("{:<30} {}", uri, modified);
        } else {
            println!("{}", uri);
        }
    }

    fn write_blob(&self, uri: &str, size: &str, content_type: &str, modified: &str, long: bool) {
        if long {
            println!("{:<10} {:<15} {:<20} {}", size, content_type, modified, uri);
        } else {
            println!("{}", uri);
        }
    }

    fn write_prefix(&self, uri: &str, long: bool) {
        if long {
            println!("{:<10} {:<15} {:<20} {}", "-", "DIR", "-", uri);
        } else {
            println!("{}", uri);
        }
    }

    fn write_local_file(&self, name: &str, size: &str, file_type: &str, long: bool) {
        if long {
            println!("{:<10} {:<10} {}", size, file_type, name);
        } else {
            println!("{}", name);
        }
    }
}

/// Factory function to create the appropriate writer based on output destination
pub fn create_writer() -> Box<dyn OutputWriter> {
    if io::stdout().is_terminal() {
        Box::new(TtyWriter)
    } else {
        Box::new(PlainWriter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tty_writer_creates_colored_output() {
        // This is more of a smoke test - actual output verification would need
        // a test writer that captures output
        let writer = TtyWriter;
        writer.write_header("Test Header");
        // If this doesn't panic, it works
    }

    #[test]
    fn test_plain_writer_creates_plain_output() {
        let writer = PlainWriter;
        writer.write_header("Test Header");
        // If this doesn't panic, it works
    }
}

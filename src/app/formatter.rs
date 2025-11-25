use crate::app::models::FileEntry;
use anyhow::Result;
use std::fs;

pub struct OutputGenerator;

impl OutputGenerator {
    pub fn generate_tree(entries: &[FileEntry]) -> String {
        let mut output = String::new();

        for entry in entries {
            let indent = "    ".repeat(entry.depth.saturating_sub(1));
            let name = entry.path.file_name().unwrap_or_default().to_string_lossy();

            let marker = if entry.is_dir { "/" } else { "" };
            output.push_str(&format!("{}{}{}\n", indent, name, marker));
        }

        output.trim_end().to_string()
    }


    pub fn generate_content(entries: &[FileEntry]) -> String {
        let mut blocks = Vec::new();

        for entry in entries {
            if entry.include_content {
                match fs::read_to_string(&entry.path) {
                    Ok(content) => {
                        blocks.push(format!(
                            "<file path=\"{}\">\n{}\n</file>",
                            entry.relative_path, content
                        ));
                    }
                    Err(e) => {
                        blocks.push(format!(
                            "<file path=\"{}\" error=\"true\">Error reading file: {}</file>",
                            entry.relative_path, e
                        ));
                    }
                }
            }
        }

        blocks.join("\n\n")
    }

    pub fn format_full_output(tree: &str, content: &str) -> String {
        let mut out = String::from("<directory_structure>\n");
        out.push_str(tree);
        out.push_str("\n</directory_structure>");

        if !content.is_empty() {
            out.push_str("\n\n<file_contents>\n");
            out.push_str(content);
            out.push_str("\n</file_contents>");
        }

        out
    }
}
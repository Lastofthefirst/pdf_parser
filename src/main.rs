use anyhow::{Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{error, info, warn};
use walkdir::WalkDir;

/// PDF Parser - Extract and flatten content from PDFs using marker
#[derive(Parser, Debug)]
#[command(name = "pdf_parser")]
#[command(about = "A tool to extract and flatten PDF content using marker")]
struct Args {
    /// Input PDF file or directory containing PDFs
    #[arg(help = "Path to PDF file or directory")]
    input: PathBuf,

    /// Output directory (defaults to input_name with json_ prefix)
    #[arg(short, long, help = "Override output directory")]
    output_dir: Option<PathBuf>,

    /// Path to marker_single binary
    #[arg(long, default_value = "/home/runner/.local/bin/marker_single")]
    marker_path: PathBuf,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

/// Represents a flattened block from marker's JSON output
#[derive(Debug, Serialize, Deserialize, Clone)]
struct FlattenedBlock {
    /// Unique identifier for the block
    pub id: String,
    /// Type of block (e.g., Text, SectionHeader, etc.)
    pub block_type: String,
    /// HTML content of the block
    pub html: Option<String>,
    /// Plain text content (if available)
    pub text: Option<String>,
    /// Bounding box coordinates
    pub polygon: Option<Vec<[f64; 2]>>,
}

/// Represents marker's JSON output structure
#[derive(Debug, Deserialize)]
struct MarkerDocument {
    /// List of pages in the document
    pub children: Option<Vec<MarkerBlock>>,
}

/// Represents a block in marker's hierarchical structure
#[derive(Debug, Deserialize)]
struct MarkerBlock {
    /// Unique identifier
    pub id: String,
    /// Block type
    pub block_type: String,
    /// HTML content
    pub html: Option<String>,
    /// Bounding box
    pub polygon: Option<Vec<[f64; 2]>>,
    /// Child blocks
    pub children: Option<Vec<MarkerBlock>>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    
    // Initialize logging
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(if args.verbose {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        })
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting PDF parser");
    info!("Input: {:?}", args.input);
    
    // Process input
    if args.input.is_file() {
        process_single_pdf(&args)?;
    } else if args.input.is_dir() {
        process_directory(&args)?;
    } else {
        anyhow::bail!("Input path does not exist or is not a file/directory");
    }

    info!("PDF processing completed successfully");
    Ok(())
}

/// Process a single PDF file
fn process_single_pdf(args: &Args) -> Result<()> {
    let input_path = &args.input;
    
    // Validate input is a PDF
    if !input_path.extension().map_or(false, |ext| ext == "pdf") {
        anyhow::bail!("Input file must have .pdf extension");
    }

    info!("Processing PDF: {:?}", input_path);

    // Determine output directory
    let output_dir = determine_output_dir(args, input_path)?;
    std::fs::create_dir_all(&output_dir)
        .context("Failed to create output directory")?;

    // Process the PDF
    let flattened_blocks = extract_and_flatten_pdf(input_path, &args.marker_path)?;
    
    // Save results
    let output_file = output_dir.join(format!(
        "{}.json",
        input_path.file_stem().unwrap().to_string_lossy()
    ));
    
    save_flattened_blocks(&flattened_blocks, &output_file)?;
    
    info!("Saved {} blocks to {:?}", flattened_blocks.len(), output_file);
    Ok(())
}

/// Process all PDFs in a directory recursively
fn process_directory(args: &Args) -> Result<()> {
    info!("Processing directory: {:?}", args.input);
    
    let pdf_files: Vec<_> = WalkDir::new(&args.input)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry.path().extension().map_or(false, |ext| ext == "pdf")
        })
        .collect();

    if pdf_files.is_empty() {
        warn!("No PDF files found in directory");
        return Ok(());
    }

    info!("Found {} PDF files", pdf_files.len());

    for pdf_file in pdf_files {
        let pdf_path = pdf_file.path();
        info!("Processing: {:?}", pdf_path);
        
        // Create a temporary args struct for this file
        let file_args = Args {
            input: pdf_path.to_path_buf(),
            output_dir: args.output_dir.clone(),
            marker_path: args.marker_path.clone(),
            verbose: args.verbose,
        };
        
        if let Err(e) = process_single_pdf(&file_args) {
            error!("Failed to process {:?}: {}", pdf_path, e);
            // Continue processing other files
        }
    }

    Ok(())
}

/// Determine the output directory based on input and args
fn determine_output_dir(args: &Args, input_path: &Path) -> Result<PathBuf> {
    if let Some(output_dir) = &args.output_dir {
        Ok(output_dir.clone())
    } else {
        // Create default output directory with json_ prefix
        let parent = input_path.parent().unwrap_or(Path::new("."));
        let name = if input_path.extension().map_or(false, |ext| ext == "pdf") || input_path.is_file() {
            // For PDF files, use the stem (filename without extension)
            input_path.file_stem().unwrap().to_string_lossy()
        } else {
            // For directories, use the directory name
            input_path.file_name().unwrap().to_string_lossy()
        };
        Ok(parent.join(format!("json_{}", name)))
    }
}

/// Extract content from PDF using marker and flatten the structure
fn extract_and_flatten_pdf(pdf_path: &Path, marker_path: &Path) -> Result<Vec<FlattenedBlock>> {
    info!("Extracting content from PDF using marker");
    
    // Create temporary directory for marker output
    let temp_dir = std::env::temp_dir().join("pdf_parser_temp");
    std::fs::create_dir_all(&temp_dir)?;
    
    // Call marker_single to extract JSON
    let output = Command::new(marker_path)
        .arg(pdf_path)
        .arg("--output_format")
        .arg("json")
        .arg("--output_dir")
        .arg(&temp_dir)
        .output()
        .context("Failed to execute marker_single")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("marker_single failed: {}", stderr);
    }

    // Find the generated JSON file
    let json_file = temp_dir.join(format!(
        "{}.json",
        pdf_path.file_stem().unwrap().to_string_lossy()
    ));

    if !json_file.exists() {
        anyhow::bail!("Marker did not generate expected JSON output file");
    }

    // Parse the JSON
    let json_content = std::fs::read_to_string(&json_file)
        .context("Failed to read marker JSON output")?;
    
    let document: MarkerDocument = serde_json::from_str(&json_content)
        .context("Failed to parse marker JSON output")?;

    // Flatten the hierarchical structure
    let flattened = flatten_document(&document);
    
    // Clean up temporary file
    let _ = std::fs::remove_file(&json_file);
    
    info!("Extracted and flattened {} blocks", flattened.len());
    Ok(flattened)
}

/// Flatten the hierarchical marker document structure into a simple array
fn flatten_document(document: &MarkerDocument) -> Vec<FlattenedBlock> {
    let mut flattened = Vec::new();
    
    if let Some(children) = &document.children {
        for child in children {
            flatten_block_recursive(child, &mut flattened);
        }
    }
    
    // Filter out non-content blocks
    flattened.retain(|block| {
        !matches!(block.block_type.as_str(), "Page" | "Header" | "Footer")
    });
    
    flattened
}

/// Recursively flatten a block and its children
fn flatten_block_recursive(block: &MarkerBlock, flattened: &mut Vec<FlattenedBlock>) {
    // Extract text content from HTML if available
    let text = block.html.as_ref().and_then(|html| {
        // Simple HTML tag stripping - in a real implementation, 
        // you might want to use a proper HTML parser
        let text = html.replace("<br>", "\n")
            .replace("<p>", "")
            .replace("</p>", "\n");
        if text.trim().is_empty() {
            None
        } else {
            Some(strip_html_tags(&text))
        }
    });

    let flat_block = FlattenedBlock {
        id: block.id.clone(),
        block_type: block.block_type.clone(),
        html: block.html.clone(),
        text,
        polygon: block.polygon.clone(),
    };
    
    flattened.push(flat_block);
    
    // Process children recursively
    if let Some(children) = &block.children {
        for child in children {
            flatten_block_recursive(child, flattened);
        }
    }
}

/// Simple HTML tag stripping function
fn strip_html_tags(html: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    
    result.trim().to_string()
}

/// Save flattened blocks to JSON file
fn save_flattened_blocks(blocks: &[FlattenedBlock], output_path: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(blocks)
        .context("Failed to serialize flattened blocks")?;
    
    std::fs::write(output_path, json)
        .context("Failed to write output file")?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_flatten_document() {
        let document_json = json!({
            "children": [
                {
                    "id": "/page/1/Page/0",
                    "block_type": "Page",
                    "html": null,
                    "polygon": [[0.0, 0.0], [612.0, 0.0], [612.0, 792.0], [0.0, 792.0]],
                    "children": [
                        {
                            "id": "/page/1/SectionHeader/1",
                            "block_type": "SectionHeader",
                            "html": "<h1>Introduction</h1>",
                            "polygon": [[100.0, 100.0], [200.0, 100.0], [200.0, 130.0], [100.0, 130.0]],
                            "children": null
                        },
                        {
                            "id": "/page/1/Text/2",
                            "block_type": "Text",
                            "html": "<p>This is some sample text content.</p>",
                            "polygon": [[100.0, 150.0], [400.0, 150.0], [400.0, 200.0], [100.0, 200.0]],
                            "children": null
                        }
                    ]
                }
            ]
        });

        let document: MarkerDocument = serde_json::from_value(document_json).unwrap();
        let flattened = flatten_document(&document);

        // Should exclude Page block but include SectionHeader and Text
        assert_eq!(flattened.len(), 2);
        
        assert_eq!(flattened[0].block_type, "SectionHeader");
        assert_eq!(flattened[0].text, Some("Introduction".to_string()));
        
        assert_eq!(flattened[1].block_type, "Text");
        assert_eq!(flattened[1].text, Some("This is some sample text content.".to_string()));
    }

    #[test]
    fn test_strip_html_tags() {
        assert_eq!(strip_html_tags("<h1>Hello World</h1>"), "Hello World");
        assert_eq!(strip_html_tags("<p>Text with <em>emphasis</em></p>"), "Text with emphasis");
        assert_eq!(strip_html_tags("Plain text"), "Plain text");
        assert_eq!(strip_html_tags(""), "");
    }

    #[test]
    fn test_determine_output_dir() {
        // Test with explicit output directory
        let args_with_output = Args {
            input: PathBuf::from("/path/to/document.pdf"),
            output_dir: Some(PathBuf::from("/custom/output")),
            marker_path: PathBuf::from("marker_single"),
            verbose: false,
        };

        let output_dir = determine_output_dir(&args_with_output, &args_with_output.input).unwrap();
        assert_eq!(output_dir, PathBuf::from("/custom/output"));

        // Test with default output (file case) - we need to manually specify that it's a file
        let pdf_path = PathBuf::from("/path/to/document.pdf");
        let args = Args {
            input: pdf_path.clone(),
            output_dir: None,
            marker_path: PathBuf::from("marker_single"),
            verbose: false,
        };

        // Since we can't rely on is_file() in tests with non-existent paths,
        // we'll test the logic directly by checking the file stem
        let parent = pdf_path.parent().unwrap();
        let stem = pdf_path.file_stem().unwrap().to_string_lossy();
        let expected = parent.join(format!("json_{}", stem));
        
        let output_dir = determine_output_dir(&args, &pdf_path).unwrap();
        assert_eq!(output_dir, expected);
    }
}

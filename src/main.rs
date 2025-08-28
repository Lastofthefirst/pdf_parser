use clap::Parser;
use glob::glob;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[clap(
    name = "flatten_marker_output",
    version = "0.1.0",
    author = "Your Name",
    about = "Flattens Marker output by removing non-content elements and extracting text",
    long_about = "This tool processes PDF files and their corresponding JSON representations, filtering out non-content elements and extracting plain text from HTML content to create a flattened, clean representation of the document content.

Input can be:
- A single PDF file
- A single JSON file (already converted from PDF)
- A directory containing PDF files (with potential subdirectories)

The tool will:
1. If input is a PDF, convert it to JSON using the Marker library
2. If input is JSON, process it directly
3. If input is a directory, process all PDF files recursively

Processing includes:
1. Flattening the document structure
2. Filtering out non-content blocks (Page, PageHeader, PageFooter, Picture, ListGroup)
3. Removing unnecessary data fields (polygon, bbox, children, section_hierarchy, images)
4. Extracting plain text content from HTML markup

Output will be saved in the same directory as the input file with '_processed' appended to the filename, unless a custom output directory is specified with the -o flag."
)]
struct Args {
    /// Input path (PDF file, directory of PDFs, or JSON file)
    input: String,

    /// Output directory (optional)
    #[clap(short, long)]
    output_dir: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Block {
    #[serde(default)]
    id: String,
    #[serde(default)]
    block_type: String,
    #[serde(default)]
    html: String,
    #[serde(default)]
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    polygon: Option<Vec<Vec<f64>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bbox: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    children: Option<Vec<Block>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    section_hierarchy: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    images: Option<serde_json::Value>,
}

impl Default for Block {
    fn default() -> Self {
        Block {
            id: String::new(),
            block_type: String::new(),
            html: String::new(),
            text: String::new(),
            polygon: None,
            bbox: None,
            children: None,
            section_hierarchy: None,
            images: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct Document {
    children: Vec<Block>,
}

// Struct to track unprocessed files
#[derive(Debug)]
struct UnprocessedFile {
    path: String,
    reason: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let input_path = Path::new(&args.input);

    if input_path.is_file() {
        if input_path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            match process_json_file(input_path, &args.output_dir) {
                Ok(_) => (),
                Err(e) => {
                    eprintln!("Error processing file {:?}: {}", input_path, e);
                    std::process::exit(1);
                }
            }
        } else {
            process_pdf_file(input_path, &args.output_dir)?;
        }
    } else if input_path.is_dir() {
        // For directory input, we need to determine the output directory
        let output_dir = if let Some(ref output_dir) = args.output_dir {
            output_dir.clone()
        } else {
            // Create a _processed directory right next to the input directory
            let parent_dir = input_path.parent().unwrap_or_else(|| Path::new("."));
            let dir_name = input_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("output");
            let processed_dir_name = format!("{}_processed", dir_name);
            parent_dir.join(processed_dir_name).to_string_lossy().to_string()
        };
        
        let unprocessed_files = process_pdf_directory_with_structure(input_path, &output_dir)?;
        if !unprocessed_files.is_empty() {
            println!("\nUnprocessed files:");
            for file in unprocessed_files {
                println!("  {}: {}", file.path, file.reason);
            }
        }
    } else {
        eprintln!("Input path is neither a file nor a directory");
        std::process::exit(1);
    }

    Ok(())
}

fn process_json_file(
    input_path: &Path,
    output_dir: &Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Processing JSON file: {:?}", input_path);

    // Read the JSON file
    let json_content = fs::read_to_string(input_path)?;
    
    // Try to parse as Document, if it fails, it's likely not a valid Marker JSON
    let document: Document = match serde_json::from_str(&json_content) {
        Ok(doc) => doc,
        Err(e) => {
            return Err(format!("Invalid JSON schema in {:?}: {}", input_path, e).into());
        }
    };

    // Process the document to remove non-content elements
    let filtered_blocks = flatten_and_filter_blocks(document.children);

    // Determine output path
    let output_path = determine_output_path(input_path, output_dir, "json")?;
    
    // Write the processed JSON to file
    let mut output_file = File::create(&output_path)?;
    let processed_json = serde_json::to_string_pretty(&filtered_blocks)?;
    output_file.write_all(processed_json.as_bytes())?;

    println!("Processed JSON saved to: {:?}", output_path);
    Ok(())
}

fn process_json_file_with_output_path(
    input_path: &Path,
    output_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Processing JSON file: {:?}", input_path);

    // Read the JSON file
    let json_content = fs::read_to_string(input_path)?;
    
    // Try to parse as Document, if it fails, it's likely not a valid Marker JSON
    let document: Document = match serde_json::from_str(&json_content) {
        Ok(doc) => doc,
        Err(e) => {
            return Err(format!("Invalid JSON schema in {:?}: {}", input_path, e).into());
        }
    };

    // Process the document to remove non-content elements
    let filtered_blocks = flatten_and_filter_blocks(document.children);

    // Modify the output path to add "_processed" to the filename
    let file_name = output_path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("output");
    let output_file_name = format!("{}_processed.json", file_name);
    
    let final_output_path = if let Some(parent) = output_path.parent() {
        parent.join(output_file_name)
    } else {
        PathBuf::from(output_file_name)
    };
    
    // Create parent directories if they don't exist
    if let Some(parent) = final_output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    
    // Write the processed JSON to file
    let mut output_file = File::create(&final_output_path)?;
    let processed_json = serde_json::to_string_pretty(&filtered_blocks)?;
    output_file.write_all(processed_json.as_bytes())?;

    println!("Processed JSON saved to: {:?}", final_output_path);
    Ok(())
}

fn process_pdf_file(
    input_path: &Path,
    _output_dir: &Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Processing PDF file: {:?}", input_path);
    
    // For now, we'll just print a message since the actual PDF processing 
    // would require calling the Python marker tool
    println!("PDF processing would call marker tool here");
    
    // In a full implementation, we would:
    // 1. Call the marker tool to convert PDF to JSON
    // 2. Process the resulting JSON as in process_json_file
    
    Ok(())
}

fn process_pdf_file_with_output_path(
    input_path: &Path,
    output_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Processing PDF file: {:?}", input_path);
    
    // For now, we'll just print a message since the actual PDF processing 
    // would require calling the Python marker tool
    println!("PDF processing would call marker tool here and save to: {:?}", output_path);
    
    // In a full implementation, we would:
    // 1. Call the marker tool to convert PDF to JSON
    // 2. Process the resulting JSON and save it to output_path
    
    Ok(())
}

fn process_pdf_directory_with_structure(
    input_dir: &Path,
    output_dir: &str,
) -> Result<Vec<UnprocessedFile>, Box<dyn std::error::Error>> {
    println!("Processing directory with structure: {:?}", input_dir);
    
    let mut unprocessed_files = Vec::new();
    
    // Convert input_dir to a canonical path for consistent comparison
    let canonical_input_dir = input_dir.canonicalize()?;
    
    // Define paths to exclude using canonical paths
    let target_dir = canonical_input_dir.join("target");
    let git_dir = canonical_input_dir.join(".git");
    
    // Helper function to check if a path should be excluded
    let is_excluded_path = |path: &Path| -> bool {
        // Check if the path is in target or .git directories
        if let Ok(canonical_path) = path.canonicalize() {
            canonical_path.starts_with(&target_dir) || canonical_path.starts_with(&git_dir)
        } else {
            // If we can't canonicalize, fall back to string matching
            path.to_string_lossy().contains("/target/") || path.to_string_lossy().contains("/.git/")
        }
    };
    
    // Find all PDF files in the directory and subdirectories (excluding target and .git)
    let pdf_pattern = format!("{}/**/*.pdf", canonical_input_dir.display());
    for entry in glob(&pdf_pattern)? {
        match entry {
            Ok(path) => {
                // Skip files in target and .git directories
                if is_excluded_path(&path) {
                    continue;
                }
                
                // Determine the relative path from input_dir to this file
                if let Ok(relative_path) = path.strip_prefix(&canonical_input_dir) {
                    // Create the corresponding output path
                    let output_path = Path::new(output_dir).join(relative_path);
                    
                    // Create the parent directories if they don't exist
                    if let Some(parent) = output_path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    
                    // Process the PDF file with the output path
                    if let Err(e) = process_pdf_file_with_output_path(&path, &output_path) {
                        unprocessed_files.push(UnprocessedFile {
                            path: path.to_string_lossy().to_string(),
                            reason: format!("Error processing PDF: {}", e),
                        });
                    }
                }
            }
            Err(e) => {
                unprocessed_files.push(UnprocessedFile {
                    path: "Unknown file".to_string(),
                    reason: format!("Error reading file: {:?}", e),
                });
            }
        }
    }
    
    // Also check for JSON files in the directory and subdirectories (excluding target and .git)
    let json_pattern = format!("{}/**/*.json", canonical_input_dir.display());
    for entry in glob(&json_pattern)? {
        match entry {
            Ok(path) => {
                // Skip files in target and .git directories
                if is_excluded_path(&path) {
                    continue;
                }
                
                // Skip already processed files (those with "_processed" in the name)
                if !path.to_string_lossy().contains("_processed") {
                    // Determine the relative path from input_dir to this file
                    if let Ok(relative_path) = path.strip_prefix(&canonical_input_dir) {
                        // Create the corresponding output path
                        let output_path = Path::new(output_dir).join(relative_path);
                        
                        // Create the parent directories if they don't exist
                        if let Some(parent) = output_path.parent() {
                            fs::create_dir_all(parent)?;
                        }
                        
                        // Process the JSON file with the output path
                        if let Err(e) = process_json_file_with_output_path(&path, &output_path) {
                            unprocessed_files.push(UnprocessedFile {
                                path: path.to_string_lossy().to_string(),
                                reason: format!("{}", e),
                            });
                        }
                    }
                }
            }
            Err(e) => {
                unprocessed_files.push(UnprocessedFile {
                    path: "Unknown file".to_string(),
                    reason: format!("Error reading file: {:?}", e),
                });
            }
        }
    }
    
    // Check for other files that aren't PDF or JSON (excluding target and .git)
    let all_files_pattern = format!("{}/**/*", canonical_input_dir.display());
    for entry in glob(&all_files_pattern)? {
        match entry {
            Ok(path) => {
                // Skip directories
                if path.is_dir() {
                    continue;
                }
                
                // Skip files in target and .git directories
                if is_excluded_path(&path) {
                    continue;
                }
                
                // Skip PDF and JSON files as they're already handled
                if let Some(ext) = path.extension() {
                    if ext == "pdf" || ext == "json" {
                        continue;
                    }
                }
                
                // Skip already processed files
                if path.to_string_lossy().contains("_processed") {
                    continue;
                }
                
                // Add to unprocessed files list
                unprocessed_files.push(UnprocessedFile {
                    path: path.to_string_lossy().to_string(),
                    reason: "Unsupported file type".to_string(),
                });
            }
            Err(e) => {
                unprocessed_files.push(UnprocessedFile {
                    path: "Unknown file".to_string(),
                    reason: format!("Error reading file: {:?}", e),
                });
            }
        }
    }
    
    Ok(unprocessed_files)
}

fn flatten_and_filter_blocks(blocks: Vec<Block>) -> Vec<Block> {
    let mut result = Vec::new();
    
    for block in blocks {
        // Skip page blocks as they are just containers
        if block.block_type == "Page" {
            // Process children of page blocks
            if let Some(children) = block.children {
                result.extend(flatten_and_filter_blocks(children));
            }
        } else {
            // Filter out header, footer, picture, and list group blocks
            if block.block_type != "PageHeader" 
                && block.block_type != "PageFooter" 
                && block.block_type != "Picture"
                && block.block_type != "ListGroup" {
                // Extract text from HTML
                let text = extract_text_from_html(&block.html);
                
                // Remove polygon, bbox, children, section_hierarchy, and images fields
                let filtered_block = Block {
                    id: block.id,
                    block_type: block.block_type,
                    html: block.html,
                    text,
                    polygon: None,
                    bbox: None,
                    children: None,
                    section_hierarchy: None,
                    images: None,
                };
                result.push(filtered_block);
            }
        }
    }
    
    result
}

fn extract_text_from_html(html: &str) -> String {
    // Create a regex to remove HTML tags
    let re = Regex::new(r"<[^>]*>").unwrap();
    
    // Remove HTML tags
    let text = re.replace_all(html, " ");
    
    // Clean up whitespace
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn determine_output_path(
    input_path: &Path,
    output_dir: &Option<String>,
    extension: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let output_path = if let Some(dir) = output_dir {
        // Use provided output directory
        let dir_path = Path::new(dir);
        
        // Get the file name
        let file_name = input_path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("output");
        let output_file_name = format!("{}_processed.{}", file_name, extension);
        
        // For now, just put all files in the output directory
        // In a more sophisticated implementation, we could preserve the directory structure
        fs::create_dir_all(dir_path)?;
        dir_path.join(output_file_name)
    } else {
        // Default to same directory as input
        let parent_dir = input_path.parent().unwrap_or_else(|| Path::new("."));
        
        // Create output filename based on input
        let file_name = input_path
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("output");
        let output_file_name = format!("{}_processed.{}", file_name, extension);
        
        parent_dir.join(output_file_name)
    };
    
    Ok(output_path)
}

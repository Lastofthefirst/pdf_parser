# PDF Parser

A Rust-based command-line tool to extract and flatten PDF content using the marker library. This tool processes PDFs to extract structured content in JSON format, then flattens the hierarchical structure into a simple array while filtering out non-content blocks.

## Features

- ✅ **Single PDF Processing**: Process individual PDF files
- ✅ **Directory Processing**: Recursively process all PDFs in a directory tree
- ✅ **Content Filtering**: Automatically filters out Page, Header, and Footer blocks
- ✅ **Structure Flattening**: Converts hierarchical block structure to a flat array
- ✅ **HTML Text Extraction**: Extracts plain text from HTML content
- ✅ **Bounding Box Preservation**: Maintains polygon coordinates for each block
- ✅ **Configurable Output**: Customizable output directory with sensible defaults
- ✅ **Comprehensive Logging**: Detailed logging with verbose mode
- ✅ **Error Handling**: Robust error handling and reporting

## Installation

### Prerequisites

1. **Rust**: Install Rust from [rustup.rs](https://rustup.rs/)
2. **marker-pdf**: Install the marker library:
   ```bash
   pip install marker-pdf
   ```

### Building

```bash
# Clone the repository
git clone <repository-url>
cd pdf_parser

# Build the project
cargo build --release

# The binary will be available at target/release/pdf_parser
```

## Usage

### Basic Usage

```bash
# Process a single PDF file
./target/release/pdf_parser document.pdf

# Process a directory of PDFs
./target/release/pdf_parser /path/to/pdf_directory

# Use custom output directory
./target/release/pdf_parser document.pdf --output-dir /custom/output

# Enable verbose logging
./target/release/pdf_parser document.pdf --verbose
```

### Command-Line Options

```
A tool to extract and flatten PDF content using marker

Usage: pdf_parser [OPTIONS] <INPUT>

Arguments:
  <INPUT>  Path to PDF file or directory

Options:
  -o, --output-dir <OUTPUT_DIR>    Override output directory
      --marker-path <MARKER_PATH>  Path to marker_single binary [default: /home/runner/.local/bin/marker_single]
  -v, --verbose                    Enable verbose logging
  -h, --help                       Print help
```

### Output Structure

The tool creates output directories with the `json_` prefix by default:

```
# For single file: document.pdf
document.pdf → json_document/document.json

# For directory: /path/to/pdfs/
/path/to/pdfs/ → /path/to/json_pdfs/
├── file1.json
├── file2.json
└── subdir/
    └── file3.json
```

### Output Format

Each processed PDF generates a JSON file containing an array of flattened content blocks:

```json
[
  {
    "id": "/page/1/SectionHeader/1",
    "block_type": "SectionHeader", 
    "html": "<h1>Document Title</h1>",
    "text": "Document Title",
    "polygon": [[72.0, 720.0], [540.0, 720.0], [540.0, 745.0], [72.0, 745.0]]
  },
  {
    "id": "/page/1/Text/2",
    "block_type": "Text",
    "html": "<p>This is a paragraph of content.</p>",
    "text": "This is a paragraph of content.",
    "polygon": [[72.0, 680.0], [540.0, 680.0], [540.0, 710.0], [72.0, 710.0]]
  }
]
```

### Block Types

The tool preserves various content block types while filtering out structural elements:

**Preserved Blocks:**
- `SectionHeader` - Document headings and section titles
- `Text` - Regular text content paragraphs
- `TextInlineMath` - Text containing mathematical expressions
- `Figure` - Image and figure references
- `Table` - Table structures
- `ListItem` - List items and bullet points

**Filtered Blocks:**
- `Page` - Page-level containers (filtered out)
- `Header` - Page headers (filtered out)
- `Footer` - Page footers (filtered out)

## Development

### Running Tests

```bash
# Run all tests
cargo test

# Run tests with verbose output
cargo test --verbose

# Run specific test
cargo test test_flatten_document
```

### Project Structure

```
src/
├── main.rs              # Main application logic
Cargo.toml               # Project dependencies
Cargo.lock              # Dependency lock file
README.md               # This documentation
tablas-de-bahaullah.pdf # Sample PDF for testing
```

### Architecture

The application follows a modular design:

1. **CLI Parsing**: Uses `clap` for robust command-line argument parsing
2. **PDF Processing**: Delegates to `marker_single` for PDF content extraction
3. **JSON Parsing**: Uses `serde_json` to parse marker's hierarchical output
4. **Flattening Logic**: Recursively traverses and flattens the block hierarchy
5. **Filtering**: Removes non-content blocks based on `block_type`
6. **Output Generation**: Serializes flattened data to JSON files

### Key Functions

- `process_single_pdf()`: Handles individual PDF file processing
- `process_directory()`: Recursively processes directories of PDFs
- `extract_and_flatten_pdf()`: Coordinates marker execution and JSON processing
- `flatten_document()`: Converts hierarchical structure to flat array
- `flatten_block_recursive()`: Recursive flattening with filtering
- `strip_html_tags()`: Simple HTML tag removal for text extraction

## Testing

The project includes comprehensive tests and a mock marker implementation for testing in environments where the full marker setup isn't available.

### Mock Testing

For development and CI environments, a mock marker script is available:

```bash
# Test with mock marker (doesn't require model downloads)
./target/debug/pdf_parser tablas-de-bahaullah.pdf --marker-path /tmp/mock_marker_single
```

The mock generates realistic test data that demonstrates all the flattening and filtering capabilities.

## Troubleshooting

### Common Issues

1. **marker_single not found**: Ensure marker-pdf is installed and accessible
2. **Model download failures**: marker requires internet access to download ML models
3. **Permission errors**: Ensure write permissions for output directories
4. **Memory issues**: Large PDFs may require increased memory allocation

### Debug Mode

Enable verbose logging to troubleshoot issues:

```bash
./target/release/pdf_parser document.pdf --verbose
```

This provides detailed information about:
- File discovery and validation
- marker execution and output
- JSON parsing and processing
- Block filtering and counting
- Output file generation

## Contributing

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure all tests pass: `cargo test`
5. Submit a pull request

## License

This project is licensed under the MIT License - see the LICENSE file for details.
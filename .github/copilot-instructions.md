# Protols - Protocol Buffers Language Server

Protols is an open-source Language Server Protocol (LSP) implementation for Protocol Buffers (proto) files, written in Rust. It provides intelligent code assistance for protobuf development, including auto-completion, diagnostics, formatting, go-to-definition, hover information, and more.

Always reference these instructions first and fallback to search or bash commands only when you encounter unexpected information that does not match the info here.

## Working Effectively

### Bootstrap and Build
- Install dependencies and build the project:
  - Rust toolchain is already available (cargo 1.89.0, rustc 1.89.0)
  - Install protoc: `sudo apt update && sudo apt install -y protobuf-compiler` -- takes 2-3 minutes. NEVER CANCEL. Installs libprotoc 3.21.12.
  - clang-format is already installed and available at `/usr/bin/clang-format` (Ubuntu clang-format version 18.1.3)
  - `cargo build --verbose` -- takes about 1 minute to complete. NEVER CANCEL. Set timeout to 90+ minutes for safety.
  - `cargo test --verbose` -- takes about 6 seconds, runs 22 tests. NEVER CANCEL. Set timeout to 30+ minutes.

### Essential Commands
- Check code formatting: `cargo fmt --check` -- takes under 1 second
- Run linter: `cargo clippy` -- takes about 15 seconds. NEVER CANCEL. Set timeout to 30+ minutes.
- Run the binary: `./target/debug/protols --help` or `./target/debug/protols --version`
- Build release version: `cargo build --release` -- takes about 1 minute. NEVER CANCEL. Set timeout to 90+ minutes.
- Test specific functionality: `cargo test <test_name>` for individual tests

### External Dependencies Verification
- **protoc (Protocol Buffers Compiler)**: Required for advanced diagnostics. Install with `sudo apt install -y protobuf-compiler`. Verify with `protoc --version`.
- **clang-format**: Required for code formatting. Already available. Verify with `clang-format --version`.

## Validation and Testing

### Manual Validation Scenarios
After making changes to the LSP functionality, ALWAYS test these scenarios:

1. **Basic Build and Test Validation**:
   - `cargo build` -- should complete in ~1 minute without errors
   - `cargo test --verbose` -- should pass all 22 tests in ~6 seconds  
   - `cargo fmt --check` -- should pass formatting check
   - `cargo clippy` -- should pass linting with no warnings
   - `./target/debug/protols --help` -- should show help message
   - `./target/debug/protols --version` -- should show version 0.12.8

2. **External Dependencies Validation**:
   - `protoc --version` -- should show libprotoc 3.21.12
   - `clang-format --version` -- should show Ubuntu clang-format version 18.1.3
   - Test protoc with sample file: `protoc sample/simple.proto --descriptor_set_out=/tmp/test.desc`
   - Test clang-format with sample file: `clang-format sample/simple.proto`

3. **LSP Functionality Testing**:
   - Test specific LSP features: `cargo test parser::hover::test::test_hover`
   - Test workspace functionality: `cargo test workspace`
   - Test with include paths: `./target/debug/protols --include-paths=/tmp,/home` (will start LSP server)
   - Verify LSP server starts correctly (shows logging directory and waits for input)

4. **Sample File Validation**:
   - Ensure sample proto files in `/sample/` directory are valid
   - Test parsing with `sample/simple.proto`, `sample/everything.proto`, `sample/test.proto`
   - Verify protoc can process sample files without errors

### CRITICAL Build and Test Timing
- **NEVER CANCEL builds or tests** - they may take longer than expected
- **cargo build**: 1 minute typical, set timeout to 90+ minutes
- **cargo test**: 6 seconds typical, set timeout to 30+ minutes  
- **cargo clippy**: 15 seconds typical, set timeout to 30+ minutes
- **External dependency installation**: 2-3 minutes, set timeout to 30+ minutes

### CI Validation Requirements
Always run these commands before committing changes:
- `cargo fmt --check` -- validates code formatting
- `cargo clippy` -- validates code quality and catches common issues
- `cargo test --verbose` -- runs full test suite
- `cargo build --release` -- ensures release build works

## Key Project Structure

### Root Directory
```
├── Cargo.toml              # Main project configuration
├── Cargo.lock              # Dependency lock file
├── README.md               # Project documentation
├── protols.toml            # LSP configuration file
├── .clang-format           # Formatting configuration for proto files
├── src/                    # Main source code
├── sample/                 # Sample proto files for testing
└── .github/workflows/      # CI/CD pipelines
```

### Important Source Files
- `src/main.rs` - Entry point, command-line argument parsing, LSP server setup
- `src/server.rs` - Core LSP server implementation
- `src/lsp.rs` - LSP message handling and protocol implementation
- `src/parser/` - Tree-sitter based proto file parsing
- `src/formatter/` - Code formatting using clang-format
- `src/workspace/` - Workspace and multi-file support
- `src/config/` - Configuration management

### Key Features to Test
When modifying functionality, always validate:
- **Code Completion**: Auto-complete messages, enums, keywords
- **Diagnostics**: Syntax errors from tree-sitter and protoc
- **Document Symbols**: Navigate symbols in proto files
- **Code Formatting**: Format proto files using clang-format
- **Go to Definition**: Jump to symbol definitions
- **Hover Information**: Documentation on hover
- **Rename Symbols**: Rename and propagate changes
- **Find References**: Find symbol usage across files

## Configuration Details

### protols.toml Example
```toml
[config]
include_paths = ["src/workspace/input"]

[config.path]
clang_format = "clang-format"
protoc = "protoc"
```

### Command Line Options
- `-i, --include-paths <PATHS>`: Comma-separated include paths for proto files
- `-V, --version`: Print version information
- `-h, --help`: Print help information

## Common Development Tasks

### Common Development Tasks

### Adding New Features
1. Write tests first in appropriate `src/*/test/` directories or `src/*/input/` test data
2. Implement feature in relevant module (`src/parser/`, `src/workspace/`, `src/formatter/`, etc.)
3. Update LSP message handlers in `src/lsp.rs` if needed
4. Test with sample proto files in `sample/` directory
5. Run all validation commands before committing

### Debugging Issues
- Check logs in system temp directory (output shows location on startup: "file logging at directory: /tmp")
- Use `cargo test <module_name>` to run specific test modules
- Use `cargo test <test_name> --verbose` for detailed test output
- Test with sample files: `sample/simple.proto`, `sample/everything.proto`, `sample/test.proto`
- Test files available in `src/parser/input/` and `src/workspace/input/` for unit tests
- Verify external dependencies: `protoc --version`, `clang-format --version`

### Working with Proto Files
- Sample files available in `sample/` directory for testing
- Test input files in `src/parser/input/test_*.proto` for specific functionality
- Test workspace files in `src/workspace/input/` for multi-file scenarios  
- Always test with various proto3 syntax features: messages, enums, services, imports
- Use `protoc <file.proto> --descriptor_set_out=/tmp/test.desc` to validate proto syntax

### Performance and Timing Expectations
- Small project: ~1400 lines of Rust code
- Fast incremental builds after first build
- Test suite is comprehensive but fast (22 tests in 6 seconds)
- LSP server starts quickly but will wait for client input (normal behavior)

## Environment Notes
- This is a Rust project using edition 2024
- Uses tree-sitter for parsing proto files
- Integrates with external tools (protoc, clang-format) for enhanced functionality
- Logging goes to system temp directory with daily rotation
- Supports both Unix pipes and fallback I/O for cross-platform compatibility

## Common Command Outputs (for reference)

### Repository Structure
```
├── Cargo.toml              # Main project configuration
├── Cargo.lock              # Dependency lock file  
├── README.md               # Project documentation
├── protols.toml            # LSP configuration file
├── .clang-format           # Formatting configuration for proto files
├── LICENSE                 # MIT license
├── .gitignore              # Git ignore rules
├── src/                    # Main source code (~1400 lines)
│   ├── main.rs            # Entry point (3956 bytes)
│   ├── lsp.rs             # LSP implementation (19116 bytes)
│   ├── server.rs          # Server logic (3561 bytes)
│   ├── state.rs           # State management (9991 bytes)
│   ├── parser/            # Tree-sitter parsing
│   ├── workspace/         # Multi-file support
│   ├── formatter/         # Code formatting
│   ├── config/            # Configuration management
│   └── docs/              # Documentation generation
├── sample/                # Sample proto files
│   ├── simple.proto       # Basic examples
│   ├── everything.proto   # Comprehensive features
│   └── test.proto         # Test scenarios
└── .github/workflows/     # CI/CD pipelines
    ├── ci.yml             # Build and test
    └── release.yml        # Release automation
```

### Key Project Metadata
```
name = "protols"
description = "Language server for proto3 files"
version = "0.12.8"
edition = "2024"
license = "MIT"
```

### Test Output Summary
- **Total tests**: 22 tests across parser, workspace, config, and formatter modules
- **Test categories**: hover, definition, rename, document symbols, diagnostics, workspace operations
- **Performance**: All tests complete in under 6 seconds
- **Coverage**: Core LSP features, configuration, and multi-file workspace scenarios
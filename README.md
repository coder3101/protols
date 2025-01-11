# Protols - Protobuf Language Server

[![Crates.io](https://img.shields.io/crates/v/protols.svg)](https://crates.io/crates/protols)
[![Build and Test](https://github.com/coder3101/protols/actions/workflows/ci.yml/badge.svg)](https://github.com/coder3101/protols/actions/workflows/ci.yml)

**Protols** is an open-source Language Server Protocol (LSP) for **proto** files, powered by the robust and efficient [tree-sitter](https://tree-sitter.github.io/tree-sitter/) parser. With Protols, you get powerful code assistance for protobuf files, including auto-completion, syntax diagnostics, and more.

![](./assets/protols.mov)

## ✨ Features

- ✅ Code Completion
- ✅ Diagnostics
- ✅ Document Symbols
- ✅ Code Formatting
- ✅ Go to Definition
- ✅ Hover Information
- ✅ Rename Symbols
- ✅ Find references

## Table of Contents

- [Installation](#installation)
- [Configuration](#configuration)
  - [Basic Configuration](#basic-configuration)
  - [Experimental Configuration](#experimental-configuration)
  - [Formatter Configuration](#formatter-configuration)
- [Usage](#usage)
- [Contributing](#contributing)
- [License](#license)

---

### Installation

#### For Neovim

You can install [protols with mason.nvim](https://github.com/mason-org/mason-registry/blob/main/packages/protols/package.yaml) or directly from crates.io with:

```bash
cargo install protols
```

Then, configure it with [`nvim-lspconfig`](https://github.com/neovim/nvim-lspconfig/blob/master/doc/server_configurations.md#protols):

```lua
require'lspconfig'.protols.setup{}
```

#### For Visual Studio Code

You can use the [Protobuf Language Support](https://marketplace.visualstudio.com/items?itemName=ianandhum.protobuf-support) extension, which leverages this LSP under the hood.

> **Note:** This extension is [open source](https://github.com/ianandhum/vscode-protobuf-support) but is not maintained by us.
You can install **protols** via your preferred method, such as downloading the binary or building from source. Here’s how to get started with the simplest method:


---

## Configuration

Protols is configured using a `protols.toml` file. This configuration file can be placed in any directory, and **protols** will look for the closest file by recursively searching through parent directories.

Here’s a sample configuration:

### Sample `protols.toml`

```toml
[config] # Base configuration; these are considered stable and should not change
include_paths = ["foobar", "bazbaaz"] # Include paths to look for protofiles during parsing
disable_parse_diagnostics = true # Disable diagnostics for parsing

[config.experimental] # Experimental configuration; this should be considered unsafe and not fully tested
use_protoc_diagnostics = true # Use diagnostics from protoc

[formatter] # Formatter specific configuration
clang_format_path = "/usr/bin/clang-format" # clang-format binary to execute in formatting
```

### Configuration Sections

#### Basic Configuration

The `[config]` section contains stable settings that should generally remain unchanged.

- `include_paths`: List the directories where your `.proto` files are located.
- `disable_parse_diagnostics`: Set to `true` to disable parsing diagnostics.

#### Experimental Configuration

The `[config.experimental]` section contains settings that are still under development or are not fully tested.

- `use_protoc_diagnostics`: If set to `true`, this will enable diagnostics from the `protoc` compiler.

#### Formatter Configuration

The `[formatter]` section configures how the formatting is done.

- `clang_format_path`: Path to the `clang-format` binary used for formatting `.proto` files.

### Multiple Configuration Files

You can use multiple `protols.toml` files across different directories, and **protols** will use the closest configuration file found by traversing the parent directories.

---

## 🛠️ Usage

### Code Completion

Protols provides intelligent autocompletion for messages, enums, and proto3 keywords within the current package.

### Diagnostics

Diagnostics are powered by the tree-sitter parser, which catches syntax errors but does not utilize `protoc` for more advanced error reporting.

### Code Formatting

Formatting is enabled if [clang-format](https://clang.llvm.org/docs/ClangFormat.html) is available. You can control the [formatting style](https://clang.llvm.org/docs/ClangFormatStyleOptions.html) by placing a `.clang-format` file in the root of your workspace. Both document and range formatting are supported.

### Document Symbols

Provides symbols for the entire document, including nested symbols, messages, and enums.

### Go to Definition

Jump to the definition of any custom symbol, even across package boundaries.

### Hover Information

Displays comments and documentation for protobuf symbols on hover. Works seamlessly across package boundaries.

### Rename Symbols

Allows renaming of symbols like messages and enums, along with all their usages across packages. Currently, renaming fields within symbols is not supported directly.

### Find References

Allows user defined types like messages and enums can be checked for references. Nested fields are completely supported.

---

## Contributing

We welcome contributions from the community! If you’d like to help improve **protols**, here’s how you can get started:

1. Fork the repository and clone it to your machine.
2. Make your changes in a new branch.
3. Run the tests to ensure everything works properly.
4. Open a pull request with a description of the changes.

### Setting Up Locally

1. Clone the repository:
   ```bash
   git clone https://github.com/coder3101/protols.git
   cd protols
   ```

2. Build and test your changes:
   ```bash
   cargo build
   cargo test
   ```
---

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

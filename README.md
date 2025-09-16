# Protols - Protobuf Language Server

[![Crates.io](https://img.shields.io/crates/v/protols.svg)](https://crates.io/crates/protols)  
[![Build and Test](https://github.com/coder3101/protols/actions/workflows/ci.yml/badge.svg)](https://github.com/coder3101/protols/actions/workflows/ci.yml)

**Protols** is an open-source, feature-rich [Language Server Protocol (LSP)](https://microsoft.github.io/language-server-protocol/) for **Protocol Buffers (proto)** files. Powered by the efficient [tree-sitter](https://tree-sitter.github.io/tree-sitter/) parser, Protols offers intelligent code assistance for protobuf development, including features like auto-completion, diagnostics, formatting, and more.

## ✨ Features

- ✅ **Code Completion**: Auto-complete messages, enums, and keywords in your `.proto` files.
- ✅ **Diagnostics**: Syntax errors, import error with tree-sitter and advanced diagnostics from `protoc`.
- ✅ **Document Symbols**: Navigate and view all symbols, including messages and enums.
- ✅ **Code Formatting**: Format `.proto` files using `clang-format` for a consistent style.
- ✅ **Go to Definition**: Jump to the definition of symbols like messages or enums and imports.
- ✅ **Hover Information**: Get detailed information and documentation on hover.
- ✅ **Rename Symbols**: Rename protobuf symbols and propagate changes across the codebase.
- ✅ **Find References**: Find where messages, enums, and fields are used throughout the codebase.

---

## Table of Contents

- [Installation](#installation)
  - [For Neovim](#for-neovim)
  - [Command Line Options](#command-line-options)
  - [For Visual Studio Code](#for-visual-studio-code)
- [Configuration](#configuration)
  - [Basic Configuration](#basic-configuration)
  - [Path Configuration](#path-configuration)
- [Usage](#usage)
  - [Code Completion](#code-completion)
  - [Diagnostics](#diagnostics)
  - [Code Formatting](#code-formatting)
  - [Document Symbols](#document-symbols)
  - [Go to Definition](#go-to-definition)
  - [Hover Information](#hover-information)
  - [Rename Symbols](#rename-symbols)
  - [Find References](#find-references)
- [Contributing](#contributing)
  - [Setting Up Locally](#setting-up-locally)
- [License](#license)

---

## 🚀 Installation

### For Neovim

You can install **Protols** via [mason.nvim](https://github.com/mason-org/mason-registry/blob/main/packages/protols/package.yaml), or install it directly from [crates.io](https://crates.io/crates/protols):

```bash
cargo install protols
```

Then, configure it in your `init.lua` using [nvim-lspconfig](https://github.com/neovim/nvim-lspconfig):

```lua
require'lspconfig'.protols.setup{}
```

#### Setting Include Paths in Neovim

For dynamic configuration of include paths, you can use the `before_init` callback to set them via `initializationParams`:

```lua
require'lspconfig'.protols.setup{
  before_init = function(_, config)
    config.init_options = {
      include_paths = {
        "/usr/local/include/protobuf",
        "vendor/protos",
        "../shared-protos"
      }
    }
  end
}
```

### Command Line Options

Protols supports various command line options to customize its behavior:

```
protols [OPTIONS]

Options:
  -i, --include-paths <INCLUDE_PATHS>  Include paths for proto files, comma-separated
  -V, --version                        Print version information
  -h, --help                           Print help information
```

For example, to specify include paths when starting the language server:

```bash
protols --include-paths=/path/to/protos,/another/path/to/protos
```

### For Visual Studio Code

If you're using Visual Studio Code, you can install the [Protobuf Language Support](https://marketplace.visualstudio.com/items?itemName=ianandhum.protobuf-support) extension, which uses this LSP under the hood.

> **Note**: This extension is [open source](https://github.com/ianandhum/vscode-protobuf-support), but is not officially maintained by us.

---

## ⚙️ Configuration

Protols can be configured using a `protols.toml` file, which you can place in root of your project directory.

### Sample `protols.toml`

```toml
[config]
include_paths = ["foobar", "bazbaaz"] # Include paths to look for protofiles during parsing

[config.path]
clang_format = "clang-format"
protoc = "protoc"
```

### Configuration Sections

#### Basic Configuration

The `[config]` section contains stable settings that should generally remain unchanged.

- `include_paths`: These are directories where `.proto` files are searched. Paths can be absolute or relative to the LSP workspace root, which is already included in the `include_paths`. You can also specify include paths using:
  - **Configuration file**: Workspace-specific paths defined in `protols.toml`
  - **Command line**: Global paths using `--include-paths` flag that apply to all workspaces
  - **Initialization parameters**: Dynamic paths set via LSP `initializationParams` (useful for editors like Neovim)
  
  All include paths from these sources are combined when resolving proto imports.

#### Path Configuration

The `[config.path]` section contains path for various tools used by LSP.

- `clang_format`: Uses clang_format from this path for formatting
- `protoc`: Uses protoc from this path for diagnostics

---

## 🛠️ Usage

Protols offers a rich set of features to enhance your `.proto` file editing experience.

### Code Completion

**Protols** offers intelligent autocompletion for messages, enums, and proto3 keywords within the current package. Simply start typing, and Protols will suggest valid completions.

### Diagnostics

Syntax errors are caught by the tree-sitter parser, which highlights issues directly in your editor. More advanced error reporting, is done by `protoc` which runs after a file saved. You must have `protoc` installed and added to your path or you can specify its path in the configuration above

### Code Formatting

Format your `.proto` files using `clang-format`. To customize the formatting style, add a `.clang-format` file to the root of your project. Both document and range formatting are supported.

### Document Symbols

Protols provides a list of symbols in the current document, including nested symbols such as messages and enums. This allows for easy navigation and reference.

### Go to Definition

Jump directly to the definition of any custom symbol or imports, including those in other files or packages. This feature works across package boundaries.

### Hover Information

Hover over any symbol or imports to get detailed documentation and comments associated with it. This works seamlessly across different packages and namespaces.

### Rename Symbols

Rename symbols like messages or enums, and Propagate the changes throughout the codebase. Currently, field renaming within symbols is not supported.

### Find References

Find all references to user-defined types like messages or enums. Nested fields are fully supported, making it easier to track symbol usage across your project.

---

## 🤝 Contributing

We welcome contributions from developers of all experience levels! To get started:

1. **Fork** the repository and clone it to your local machine.
2. Create a **new branch** for your feature or fix.
3. Run the tests to ensure everything works as expected.
4. **Open a pull request** with a detailed description of your changes.

### Setting Up Locally

1. Clone the repository:

   ```bash
   git clone https://github.com/coder3101/protols.git
   cd protols
   ```

2. Build and test the project:

   ```bash
   cargo build
   cargo test
   ```

---

## 📄 License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for more details.

---

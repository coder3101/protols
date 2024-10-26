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

## 🚀 Getting Started

### Installation

#### For Neovim

To install Protols, run:

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

---

Protols is designed to supercharge your workflow with **proto** files. We welcome contributions and feedback from the community! Feel free to check out the [repository](https://github.com/coder3101/protols) and join in on improving this tool! 🎉

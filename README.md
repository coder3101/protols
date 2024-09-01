# protols
[![Crates](https://img.shields.io/crates/v/protols.svg)](https://crates.io/crates/protols)
[![Build and Test](https://github.com/coder3101/protols/actions/workflows/ci.yml/badge.svg)](https://github.com/coder3101/protols/actions/workflows/ci.yml)

A Language Server for **proto3** files. It uses tree-sitter parser for all operations.

![](./assets/protols.mov)

## Features 
- [x] Completion
- [x] Diagnostics
- [x] Formatting
- [x] Document Symbols
- [x] Go to definition
- [x] Hover
- [x] Rename

## Installation

### Neovim
Run `cargo install protols` to install and add below to setup using [`nvim-lspconfig`](https://github.com/neovim/nvim-lspconfig/blob/master/doc/server_configurations.md#protols)

```lua
require'lspconfig'.protols.setup{}

```

### Visual Studio Code

You can install an extension called [Protobuf Language Support](https://marketplace.visualstudio.com/items?itemName=ianandhum.protobuf-support) which uses this LSP under the hood.

> NOTE: It is [open-sourced](https://github.com/ianandhum/vscode-protobuf-support) but do not own or maintain it.


## Usage

#### Completion

Out of the box you will get auto Completion for Message, Enum of current package and Completion for keywords.

#### Diagnostics

Diagnostics is not reported by executing `protoc` so do not expect a full blown diagnostic result, we use tree-sitter parse for diagnostic which only displays parser errors.

#### Formatting

Formatting is enabled only if [`clang-format`](https://clang.llvm.org/docs/ClangFormat.html) is found. You can control the [formatting style](https://clang.llvm.org/docs/ClangFormatStyleOptions.html) by putting a `.clang-format` file at the root of the workspace. Both document and rage formatting is supported.

#### Document Symbols

Symbols for the document (i.e Message and Enums) along with support for nested symbols is available.

#### Goto definition

You can jump to definition of any custom symbols even across package boundaries.

#### Hover

Protobuf is usually documented by putting comments above symbol definition and hover feature utilises this assumption to present Hover text for symbols. This also works across package boundaries.

#### Rename

You can only document symbols such as Message and Enum names and all its usages will be renamed by the LSP, we do not support renaming field within a symbol however, you can jump to definition of field and rename the symbol. Renaming is also performed across pakages.

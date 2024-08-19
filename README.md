# protols
[![Crates](https://img.shields.io/crates/v/protols.svg)](https://crates.io/crates/protols)
[![Build and Test](https://github.com/coder3101/protols/actions/workflows/ci.yml/badge.svg)](https://github.com/coder3101/protols/actions/workflows/ci.yml)

A Language Server for **proto3** files. It uses tree-sitter parser for all operations.

![](./assets/protols.mov)

## Features 
- [x] Completion (keywords, enums and messages of the package)
- [x] Diagnostics - based on sytax errors
- [x] Document Symbols for message and enums
- [x] Go to definition - across packages
- [x] Hover - across packages
- [x] Rename - in current buffer only

## Installation

### Neovim
Run `cargo install protols` to install and add below to setup using [`nvim-lspconfig`](https://github.com/neovim/nvim-lspconfig/blob/master/doc/server_configurations.md#protols)

```lua
require'lspconfig'.protols.setup{}

```

### Visual Studio Code

You can install an extension called [Protobuf Language Support](https://marketplace.visualstudio.com/items?itemName=ianandhum.protobuf-support) which uses this LSP under the hood.

> NOTE: It is [open-sourced](https://github.com/ianandhum/vscode-protobuf-support) but do not own or maintain it.

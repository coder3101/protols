# protols
A Language Server for **proto3** files. It uses tree-sitter parser for all operations and always runs in **single file mode**.

![](./assets/protols.mov)

## Features 
- [x] Hover
- [x] Go to definition
- [x] Diagnostics
- [x] Document Symbols for message and enums
- [x] Rename message, enum and rpc
- [x] Completion for proto3 keywords

## Installation

Run `cargo install protols` to install and add below to setup using [`nvim-lspconfig`](https://github.com/neovim/nvim-lspconfig/blob/master/doc/server_configurations.md#protols) until we start shipping this via Mason.

```lua
require'lspconfig'.protols.setup{}

```

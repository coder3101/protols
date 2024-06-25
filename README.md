# protols
A Language Server for **proto3** files. It uses tree-sitter parser for all operations and always runs in **single file mode**.

![](./assets/protols.mov)

## Features 
- [x] Hover
- [x] Go to definition
- [x] Diagnostics

## Installation and testing

Run `cargo install protols` to install and add below to setup using `nvim-lspconfig` until we start shipping this via Mason.

```lua
require'lspconfig'.protols.setup{}

```

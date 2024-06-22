# protols
A Language Server for **proto3** files. It uses tree-sitter parser for all operations and always runs in **single file mode**.

## Features 
- [x] Hover
- [x] Go to definition
- [x] Diagnostics

 
## Installation and testing

Clone the repository and run `cargo install protols` to install locally in your `~/.cargo/bin` and the below to your `init.lua` until we start shipping this via Mason.

```lua
local client = vim.lsp.start_client({
	name = "protols",
	cmd = { vim.fn.expand("$HOME/.cargo/bin/protols") },
})

if not client then
	vim.notify("Could not connect to protols server")
end

vim.api.nvim_create_autocmd("FileType", {
	pattern = "proto",
	callback = function()
		vim.lsp.buf_attach_client(0, client)
	end,
})

```

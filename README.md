# protols
Language server for proto files

## Testing with neovim
```lua
local client = vim.lsp.start_client({
	name = "protols",
	cmd = { "<absolute path to protols binary>" },
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

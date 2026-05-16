vim.opt.cmdheight = 4

vim.api.nvim_create_autocmd("FileType", {
  pattern = "proto",
  callback = function()
    local bin = vim.fn.getcwd() .. "/target/debug/protols"
    local debug_port = os.getenv("LSP_DEBUG_PORT")

    local lsp_config = {
      name = "protols-dev",
      root_dir = vim.fn.getcwd(),
      init_options = { include_paths = { vim.fn.getcwd() } },
      on_init = function(client)
        client.notify("$/setTrace", { value = "verbose" })
      end,
    }

    if debug_port and debug_port ~= "" then
      lsp_config.cmd = vim.lsp.rpc.connect("127.0.0.1", tonumber(debug_port))
      vim.notify("protols-dev: connecting to port " .. debug_port, vim.log.levels.INFO)
    else
      if vim.fn.executable(bin) == 1 then
        lsp_config.cmd = { bin }
      else
        vim.notify("protols-dev: binary not found. Run 'cargo build' first!", vim.log.levels.ERROR)
        return
      end
    end

    vim.lsp.start(lsp_config)
  end,
})

vim.lsp.handlers["window/logMessage"] = function(_, result)
  local levels = {
    [1] = vim.log.levels.ERROR,
    [2] = vim.log.levels.WARN,
    [3] = vim.log.levels.INFO,
    [4] = vim.log.levels.DEBUG,
    [5] = vim.log.levels.DEBUG,
  }
  local time = os.date("%H:%M:%S")
  local message = string.format("[%s] %s", time, result.message)

  local level = levels[result.type] or vim.log.levels.INFO
  
  vim.notify(message, level)
end

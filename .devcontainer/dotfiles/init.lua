vim.opt.cmdheight = 4

vim.api.nvim_create_autocmd("FileType", {
  pattern = "proto",
  callback = function()
    local bin = vim.fn.getcwd() .. "/target/debug/protols"
    if vim.fn.executable(bin) == 1 then
      vim.lsp.start({
        name = "protols-dev",
        cmd = { bin },
        root_dir = vim.fn.getcwd(),
        init_options = { include_paths = { vim.fn.getcwd() } },
        on_init = function(client)
          client.notify("$/setTrace", { value = "verbose" })
        end,
      })
    else
      vim.notify("protols-dev: binary not found. Run 'cargo build' first!", 3)
    end
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

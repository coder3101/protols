# Debugging protols

Since `protols` is a language server, debugging it directly via standard I/O can
be challenging. Furthermore, attaching a debugger to a running process inside a
Dev Container is often restricted by security policies.

The recommended way to debug is to start the server in TCP mode and connect to
it with your LSP client.

## Debugging with VS Code and Neovim

The project includes a pre-configured `launch.json` for VS Code that automates
the debugging setup.

### 1. Start the Debug Session

In VS Code, go to the **Run and Debug** view and select **"Debug protols via
TCP"**.

This will:
- Build the project in debug mode.
- Start the server listening on a TCP port (default: `7301`).
- Automatically open a new terminal with **Neovim** inside the Dev Container.

### 2. Connect Neovim

The Neovim instance in the Dev Container is pre-configured to detect the
`LSP_DEBUG_PORT` environment variable. 

1. Wait for the message in the VS Code Debug Console: `LSP server listening on TCP: 127.0.0.1:7301`.
2. In the opened Neovim terminal, open any `.proto` file (e.g., `:e sample/simple.proto`).
3. Neovim will automatically connect to the debugged server instance via the specified port.

### 3. Verify Connection
To ensure the debugger is working and the server is responding:
1. Set a breakpoint in the Rust code (e.g., in `src/parser/docsymbol.rs`).
2. In Neovim, trigger an LSP request, such as fetching document symbols:
   ```vim
   :lua vim.lsp.buf.document_symbol()
   ```
3. The debugger should hit your breakpoint in VS Code.

## Manual Debugging

If you prefer to run the components manually:

1. **Start the server:**
   ```bash
   cargo run -- --port 7301
   ```
2. **Start Neovim:**
   ```bash
   LSP_DEBUG_PORT=7301 nvim your_file.proto
   ```

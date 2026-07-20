# Protols - Protobuf Language Server

[![Crates.io](https://img.shields.io/crates/v/protols.svg)](https://crates.io/crates/protols)  
[![Build and Test](https://github.com/coder3101/protols/actions/workflows/ci.yml/badge.svg)](https://github.com/coder3101/protols/actions/workflows/ci.yml)

**Protols** is an open-source, feature-rich [Language Server Protocol (LSP)](https://microsoft.github.io/language-server-protocol/) for **Protocol Buffers (proto)** files. Powered by the efficient [tree-sitter](https://tree-sitter.github.io/tree-sitter/) parser, Protols offers intelligent code assistance for protobuf development, including features like auto-completion, diagnostics, formatting, and more.

## ✨ Features

- ✅ **Code Completion**: Auto-complete messages, enums, and keywords in your `.proto` files.
- ✅ **Diagnostics**: Syntax errors, import error with tree-sitter and advanced diagnostics from `protoc`.
- ✅ **Workspace Symbols**: Search and view all symbols across workspaces.
- ✅ **Document Symbols**: Navigate and view all symbols, including nested messages and enums.
- ✅ **Code Formatting**: Format `.proto` files using `clang-format` for a consistent style.
- ✅ **Go to Definition**: Jump to the definition of symbols like messages or enums and imports.
- ✅ **Hover Information**: Get detailed information and documentation on hover.
- ✅ **Rename Symbols**: Rename protobuf symbols and propagate changes across the codebase.
- ✅ **Find References**: Find where messages, enums, and fields are used throughout the codebase.

---

## Table of Contents

- [Installation](#-installation)
  - [For Neovim](#for-neovim)
    - [Setting Include Paths in Neovim](#setting-include-paths-in-neovim)
  - [Command Line Options](#command-line-options)
    - [Examples](#examples)
  - [For Visual Studio Code](#for-visual-studio-code)
- [Configuration](#%EF%B8%8Fconfiguration)
  - [Sample `protols.toml`](#sample-protolstoml)
  - [Configuration Sections](#configuration-sections)
    - [Basic Configuration](#basic-configuration)
    - [Path Configuration](#path-configuration)
    - [Rename Configuration](#rename-configuration)
- [Usage](docs/usage.md)
- [Protocol Buffers Well-Known Types](#protocol-buffers-well-known-types)
- [Packaging](#-packaging)
- [Contributing](#-contributing)
  - [Setting Up Locally](#setting-up-locally)
    - [Option 1: Using Dev Containers (Easiest)](#option-1-using-dev-containers-easiest)
    - [Option 2: Manual Setup](#option-2-manual-setup)
  - [Debugging](#-debugging)
- [License](#-license)

---

## 🚀 Installation

### For Neovim

You can install **Protols** via [mason.nvim](https://github.com/mason-org/mason-registry/blob/main/packages/protols/package.yaml), or install it directly from [crates.io](https://crates.io/crates/protols):

```bash
cargo install protols
```

Then, configure it in your `init.lua` using [nvim-lspconfig](https://github.com/neovim/nvim-lspconfig):

```lua
require'lspconfig'.protols.setup{}
```

#### Setting Include Paths in Neovim

For dynamic configuration of include paths, you can use the `before_init` callback to set them via `initializationParams`:

```lua
require'lspconfig'.protols.setup{
  before_init = function(_, config)
    config.init_options = {
      include_paths = {
        "/usr/local/include/protobuf",
        "vendor/protos",
        "../shared-protos"
      }
    }
  end
}
```

### Command Line Options

Protols supports various command line options to customize its behavior:

```text
Usage: protols [OPTIONS]

Options:
  -i, --include-paths <INCLUDE_PATHS>  Include paths for proto files, comma-separated (can be used multiple times)
  -h, --help                           Print help
  -V, --version                        Print version

Transport:
      --stdio          Use stdin/stdout for communication (default)
      --socket <ADDR>  Use TCP communication with a specific address and port. Examples: "192.168.1.10:5005" or "0.0.0.0:5005"
      --port <PORT>    Use TCP communication on localhost with a specific port. Example: "5005"
      --pipe <PATH>    Use Unix domain socket (Linux/macOS) or Named Pipe (Windows). Examples: "/tmp/protols.sock" or "protols-pipe" (Windows)
```

#### Examples

##### Specify include paths

You can provide include paths using a comma-separated list or by repeating the
flag:

```bash
protols --include-paths=/path/to/protos,/another/path/to/protos
# or
protols -i /path/to/protos -i /another/path/to/protos
```

##### Communication via TCP
TCP transport is useful when the language server and the IDE run in different
environments.

-   **Localhost only**: Connect within the same machine.
    ```bash
    protols --port 7301
    ```
-   **Container/Docker mode**: Listen on all interfaces to allow access from the
    host machine to the container.
    ```bash
    protols --socket 0.0.0.0:7301
    ```
-   **Specific interface**: Listen on a specific network IP.
    ```bash
    protols --socket 192.168.1.10:7301
    ```

##### Communication via Unix Domain Socket
On Linux or macOS, you can use a socket file for communication:
```bash
protols --pipe /tmp/protols.sock
```

##### Communication via Named Pipes (Windows)
On Windows, you can specify a pipe name. `protols` handles the `\\.\pipe\` prefix automatically:
```bash
protols --pipe protols-pipe
```

### For Visual Studio Code

If you're using Visual Studio Code, you can install the [Protobuf Language Support](https://marketplace.visualstudio.com/items?itemName=ianandhum.protobuf-support) extension, which uses this LSP under the hood.

> **Note**: This extension is [open source](https://github.com/ianandhum/vscode-protobuf-support), but is not officially maintained by us.

---

## ⚙️Configuration

Protols can be configured using a `protols.toml` file, which you can place in root of your project directory.

### Sample `protols.toml`

```toml
[config]
include_paths = ["foobar", "bazbaaz"] # Include paths to look for protofiles during parsing

[config.path]
clang_format = "clang-format"
protoc = "protoc"

[config.rename]
chain_rpc_request_response = false # Also rename <Rpc>Request/<Rpc>Response messages when renaming an rpc
```

### Configuration Sections

#### Basic Configuration

The `[config]` section contains stable settings that should generally remain unchanged.

- `include_paths`: These are directories where `.proto` files are searched. Paths can be absolute or relative to the LSP workspace root, which is already included in the `include_paths`. You can also specify include paths using:
  - **Configuration file**: Workspace-specific paths defined in `protols.toml`
  - **Command line**: Global paths using `--include-paths` flag that apply to all workspaces
  - **Initialization parameters**: Dynamic paths set via LSP `initializationParams` (useful for editors like Neovim)

  When a file is not found in any of the paths above, the following directories are searched:
  - **Protobuf Include Path**: the path containing the [Protocol Buffers Well-Known Types](https://protobuf.dev/reference/protobuf/google.protobuf/) as detected by [`pkg-config`](https://www.freedesktop.org/wiki/Software/pkg-config/) (requires `pkg-config` present in environment and capable of finding the installation of `protobuf`)
  - **Fallback Include Path**: the fallback include path configured at compile time


  All include paths from these sources are combined when resolving proto imports.

#### Path Configuration

The `[config.path]` section contains path for various tools used by LSP.

- `clang_format`: Uses clang_format from this path for formatting
- `protoc`: Uses protoc from this path for diagnostics

#### Rename Configuration

The `[config.rename]` section tunes rename behaviour.

- `chain_rpc_request_response` (default `false`): when enabled, renaming an `rpc`
  also renames its convention-named `<Rpc>Request` and `<Rpc>Response` messages —
  and renaming such a message renames the `rpc` and its sibling message. The chain
  only fires when the names follow the [Google API design guide](https://cloud.google.com/apis/design/naming_convention#request_and_response_messages)
  convention and the messages are used by exactly one `rpc`.

---

## 🛠 Usage

For detailed usage instructions, see the [Usage Guide](docs/usage.md).

## Protocol Buffers Well-Known Types

Protols does not ship with the [Protocol Buffers Well-Known Types](https://protobuf.dev/reference/protobuf/google.protobuf/) unless configured to do so by a distribution.
In order for features above to work for the well-known types, the well-known imports must either resolve against one of the configured import paths or the environment must contain in `PATH` a `pkg-config` executable capable of resolving the package `protobuf`.
You can verify this by running

```bash
pkg-config --modversion protobuf
```

in protols' environment.

---

## 📦 Packaging

Distributions may set an absolute include path which contains the Protocol Buffers Well-Known Types,
for example pointing to the files provided by the `protobuf` package, by compiling protols with the
environment variable `FALLBACK_INCLUDE_PATH` set to the desired path. This path will be used by the
compiled executable for resolution of any proto files that could not be resolved otherwise.

## 🤝 Contributing

We welcome contributions from developers of all experience levels! To get started:

1. **Fork** the repository and clone it to your local machine.
2. Create a **new branch** for your feature or fix.
3. Run the tests to ensure everything works as expected.
4. **Open a pull request** with a detailed description of your changes.

### Setting Up Locally

#### Option 1: Using Dev Containers (Easiest)

The project includes a pre-configured **Dev Container**, which sets up Rust,
Neovim, and all dependencies automatically.

**Prerequisites:**

- [Docker](https://docker.com) installed and running.
  - (Windows users)
  [WSL2](https://learn.microsoft.com/en-us/windows/wsl/install) must be enabled.
- An IDE that supports Dev Containers (e.g., **Visual Studio Code** with the
[Dev Containers extension](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers)).

**Steps:**

1. Open the project folder in your IDE.
2. If using Visual Studio Code, click **"Reopen in Container"** in the pop-up
notification or via the Command Palette (`F1` -> `Dev Containers: Reopen in Container`).
3. The environment will be built automatically. Once finished, you can start
developing immediately.

> [!TIP]
> A pre-configured **Neovim** is included in the container for instant LSP
testing.

#### Option 2: Manual Setup

1. Clone the repository:

   ```bash
   git clone https://github.com/coder3101/protols.git
   cd protols
   ```

2. Build and test the project:

   ```bash
   cargo build
   cargo test
   ```

### 🐞 Debugging

If you want to contribute or debug the server logic, please refer to the
[Debugging Guide](docs/debugging.md) for instructions on setting up a TCP-based
debug session with VS Code and Neovim.

---

## 📄 License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for more details.

---

# Rhythm Language Server

A Language Server Protocol (LSP) implementation for the Rhythm workflow language.

## Features

- **Diagnostics**: Real-time syntax error detection
- **Completions**: IntelliSense for keywords, built-in modules, and methods
- **Hover**: Documentation on hover for built-in APIs
- **Go to Definition**: Navigate to variable declarations
- **Find References**: Find all references to a symbol
- **Signature Help**: Parameter hints for function calls
- **Document Symbols**: List all variables in a file

## Installation

### Pre-built Binaries

Download pre-built binaries from the [releases page](https://github.com/anthropics/rhythm/releases).

Available platforms:
- Linux (x86_64, aarch64)
- macOS (x86_64, aarch64)
- Windows (x86_64)

### Building from Source

```bash
cd lsp
cargo build --release
```

The binary will be at `target/release/rhythm-lsp`.

## Usage

The language server communicates over stdin/stdout using the LSP protocol:

```bash
rhythm-lsp --stdio
```

## Editor Integration

### VS Code

Install the [Rhythm VS Code extension](../editors/vscode/README.md) which bundles the language server.

### Neovim (nvim-lspconfig)

```lua
local lspconfig = require('lspconfig')
local configs = require('lspconfig.configs')

if not configs.rhythm then
  configs.rhythm = {
    default_config = {
      cmd = { 'rhythm-lsp', '--stdio' },
      filetypes = { 'rhythm', 'flow' },
      root_dir = lspconfig.util.root_pattern('.git'),
      settings = {},
    },
  }
end

lspconfig.rhythm.setup{}
```

### Helix

Add to `~/.config/helix/languages.toml`:

```toml
[[language]]
name = "rhythm"
scope = "source.rhythm"
file-types = ["flow"]
roots = []
language-server = { command = "rhythm-lsp", args = ["--stdio"] }
```

### Emacs (lsp-mode)

```elisp
(use-package lsp-mode
  :config
  (add-to-list 'lsp-language-id-configuration '(rhythm-mode . "rhythm"))
  (lsp-register-client
    (make-lsp-client
      :new-connection (lsp-stdio-connection '("rhythm-lsp" "--stdio"))
      :major-modes '(rhythm-mode)
      :server-id 'rhythm-lsp)))
```

## Development

### Running Tests

```bash
cargo test
```

### Debug Logging

Enable debug logging by setting the `RUST_LOG` environment variable:

```bash
RUST_LOG=debug rhythm-lsp --stdio
```

## License

MIT

# Rhythm Language Support for VS Code

This extension provides rich language support for the [Rhythm](https://github.com/maxnorth/rhythm) workflow language.

## Features

- **Syntax Highlighting**: Full syntax highlighting for `.flow` files
- **IntelliSense**: Autocomplete for keywords, built-in modules, and methods
- **Hover Information**: Documentation on hover for built-in APIs
- **Diagnostics**: Real-time error detection and reporting
- **Go to Definition**: Navigate to variable declarations
- **Find References**: Find all references to a variable
- **Signature Help**: Parameter hints for function calls

## Built-in API Support

The extension provides IntelliSense for all Rhythm built-in modules:

- `Inputs` - Access workflow input parameters
- `Task` - Execute durable tasks
- `Timer` - Create delays and timers
- `Signal` - Wait for external signals
- `Workflow` - Execute nested workflows
- `Promise` - Compose multiple promises (all, any, race)
- `Math` - Mathematical utility functions

## Installation

### From VS Code Marketplace

1. Open VS Code
2. Go to Extensions (Ctrl+Shift+X)
3. Search for "Rhythm"
4. Click Install

### From VSIX

1. Download the `.vsix` file from the [releases page](https://github.com/maxnorth/rhythm/releases)
2. In VS Code, go to Extensions
3. Click the `...` menu and select "Install from VSIX..."
4. Select the downloaded file

### Building from Source

```bash
cd editors/vscode
npm install
npm run compile
npm run package
```

This will create a `.vsix` file that can be installed in VS Code.

## Configuration

| Setting | Description | Default |
|---------|-------------|---------|
| `rhythm.lsp.path` | Path to the rhythm-lsp executable | Auto-detect |
| `rhythm.lsp.trace.server` | Trace level for LSP communication | `off` |

## Requirements

The extension requires the `rhythm-lsp` language server to be installed. It will automatically search for it in:

1. The path specified in `rhythm.lsp.path` setting
2. Bundled binaries in the extension
3. System PATH

## Development

See the [Language Server README](../lsp/README.md) for information about building the language server.

## License

MIT

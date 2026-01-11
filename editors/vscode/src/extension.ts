import * as path from 'path';
import * as fs from 'fs';
import * as os from 'os';
import {
    workspace,
    ExtensionContext,
    commands,
    window,
} from 'vscode';

import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    Executable,
} from 'vscode-languageclient/node';

let client: LanguageClient | undefined;

export async function activate(context: ExtensionContext): Promise<void> {
    const lspPath = findLspExecutable(context);

    if (!lspPath) {
        window.showErrorMessage(
            'Could not find rhythm-lsp executable. Please install it or configure rhythm.lsp.path.'
        );
        return;
    }

    // Create the server options
    const serverExecutable: Executable = {
        command: lspPath,
        args: ['--stdio'],
    };

    const serverOptions: ServerOptions = {
        run: serverExecutable,
        debug: serverExecutable,
    };

    // Create the client options
    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: 'file', language: 'rhythm' }],
        synchronize: {
            fileEvents: workspace.createFileSystemWatcher('**/*.flow'),
        },
        outputChannelName: 'Rhythm Language Server',
    };

    // Create and start the client
    client = new LanguageClient(
        'rhythm',
        'Rhythm Language Server',
        serverOptions,
        clientOptions
    );

    // Register commands
    context.subscriptions.push(
        commands.registerCommand('rhythm.restartServer', async () => {
            if (client) {
                await client.stop();
                await client.start();
                window.showInformationMessage('Rhythm language server restarted.');
            }
        })
    );

    // Start the client
    try {
        await client.start();
    } catch (error) {
        window.showErrorMessage(`Failed to start Rhythm language server: ${error}`);
    }
}

export async function deactivate(): Promise<void> {
    if (client) {
        await client.stop();
    }
}


/**
 * Find the rhythm-lsp executable
 */
function findLspExecutable(context: ExtensionContext): string | undefined {
    // First, check user configuration
    const configPath = workspace.getConfiguration('rhythm.lsp').get<string>('path');
    if (configPath && fs.existsSync(configPath)) {
        return configPath;
    }

    // Determine platform-specific binary name
    const platform = os.platform();
    const arch = os.arch();

    let binaryName = 'rhythm-lsp';
    if (platform === 'win32') {
        binaryName = 'rhythm-lsp.exe';
    }

    // Resolve symlinks to get the real extension path (for local development)
    const realExtensionPath = fs.realpathSync(context.extensionPath);

    // Check for bundled binary in extension
    const bundledPaths = [
        // Platform-specific subdirectory
        path.join(context.extensionPath, 'bin', `${platform}-${arch}`, binaryName),
        // Generic bin directory
        path.join(context.extensionPath, 'bin', binaryName),
        // Server directory (for development) - use real path to handle symlinks
        path.join(realExtensionPath, '..', 'lsp', 'target', 'release', binaryName),
        path.join(realExtensionPath, '..', 'lsp', 'target', 'debug', binaryName),
    ];

    for (const bundledPath of bundledPaths) {
        if (fs.existsSync(bundledPath)) {
            return bundledPath;
        }
    }

    // Fall back to PATH
    return binaryName;
}
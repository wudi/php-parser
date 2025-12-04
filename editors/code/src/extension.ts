import * as path from 'path';
import * as fs from 'fs';
import { workspace, ExtensionContext, window } from 'vscode';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    Executable
} from 'vscode-languageclient/node';

let client: LanguageClient;

export function activate(context: ExtensionContext) {
    const config = workspace.getConfiguration('phpParserRs');
    let serverPath = config.get<string>('serverPath');

    if (!serverPath) {
        // Try to find the server in the target directory
        const extPath = context.extensionPath;
        const debugPath = path.join(extPath, '..', '..', 'target', 'debug', 'pls');
        const releasePath = path.join(extPath, '..', '..', 'target', 'release', 'pls');
        
        // Also check for Windows .exe
        const debugPathExe = debugPath + '.exe';
        const releasePathExe = releasePath + '.exe';

        if (fs.existsSync(debugPath)) {
            serverPath = debugPath;
        } else if (fs.existsSync(debugPathExe)) {
            serverPath = debugPathExe;
        } else if (fs.existsSync(releasePath)) {
            serverPath = releasePath;
        } else if (fs.existsSync(releasePathExe)) {
            serverPath = releasePathExe;
        }
    }

    if (!serverPath || !fs.existsSync(serverPath)) {
        window.showErrorMessage(`PHP Parser LSP server not found. Please build it with 'cargo build' or configure 'phpParserRs.serverPath'. Searched at: ${serverPath || 'target/debug/pls'}`);
        return;
    }

    const run: Executable = {
        command: serverPath,
        options: {
            env: {
                ...process.env,
                // RUST_BACKTRACE: '1',
            }
        }
    };

    const serverOptions: ServerOptions = {
        run,
        debug: run
    };

    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: 'file', language: 'php' }],
        synchronize: {
            fileEvents: workspace.createFileSystemWatcher('**/*.php')
        }
    };

    client = new LanguageClient(
        'phpParserRs',
        'PHP Parser RS LSP',
        serverOptions,
        clientOptions
    );

    client.start();
}

export function deactivate(): Thenable<void> | undefined {
    if (!client) {
        return undefined;
    }
    return client.stop();
}

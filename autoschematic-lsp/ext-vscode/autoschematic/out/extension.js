"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function (o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
        desc = { enumerable: true, get: function () { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function (o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function (o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function (o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function (o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.activate = activate;
const vscode = __importStar(require("vscode"));
const child_process = __importStar(require("child_process"));
const util = __importStar(require("util"));
const node_1 = require("vscode-languageclient/node");
/**
 * Checks if a command exists in the system PATH
 * @param command The command to check
 * @returns Promise<boolean> True if the command exists, false otherwise
 */
async function commandExists(command) {
    const exec = util.promisify(child_process.exec);
    try {
        // Different check commands based on platform
        const checkCommand = process.platform === 'win32'
            ? `where ${command}`
            : `which ${command}`;
        await exec(checkCommand);
        return true;
    }
    catch (error) {
        return false;
    }
}
async function activate(context) {
    // Check if autoschematic-lsp is installed
    const lspExists = await commandExists('autoschematic-lsp');
    if (!lspExists) {
        const action = await vscode.window.showInformationMessage("Autoschematic doesn't appear to be installed yet. Would you like to `cargo install` it?", "Yes", "No");
        if (action === "Yes") {
            // Open a terminal and run cargo install
            const terminal = vscode.window.createTerminal('Autoschematic Install');
            terminal.sendText('cargo install --locked autoschematic');
            terminal.show();
            vscode.window.showInformationMessage("Installing Autoschematic. The extension will be ready after installation completes.");
            return; // Exit activation until installation completes
        }
        else {
            // User declined installation
            return; // Exit activation
        }
    }
    context.subscriptions.push(vscode.commands.registerCommand('autoschematic.compareWithRemote', async (fileUri) => {
        if (!fileUri) {
            vscode.window.showErrorMessage('No file selected for comparison');
            return;
        }
        try {
            // Read the content of the selected file
            // const document = await vscode.workspace.openTextDocument(fileUri);
            // const originalContent = document.getText();
            // Create the remote content (currently a placeholder)
            //
            const remoteContent = await client.sendRequest(node_1.ExecuteCommandRequest.type, {
                command: "get",
                arguments: [fileUri.path]
            });
            const remoteUri = fileUri.with({ scheme: 'autoschematic-remote', path: fileUri.path });
            const diffTitle = `Compare ${fileUri.path.split('/').pop()} with Remote`;
            // Register a content provider for our custom scheme
            const provider = vscode.workspace.registerTextDocumentContentProvider('autoschematic-remote', {
                provideTextDocumentContent(uri) {
                    console.log(remoteContent);
                    return remoteContent;
                }
            });
            context.subscriptions.push(provider);
            vscode.commands.executeCommand('vscode.diff', fileUri, // Original file URI
                remoteUri, // Modified file URI (virtual)
                diffTitle, // Title for the diff editor
                { preview: true } // Options
            );
            ;
        }
        catch (error) {
            vscode.window.showErrorMessage(`Error comparing with remote: ${error}`);
        }
    }));
    const serverOptions = {
        run: { command: 'autoschematic-lsp', transport: node_1.TransportKind.stdio },
        debug: { command: 'autoschematic-lsp', transport: node_1.TransportKind.stdio }
    };
    const clientOptions = {
        documentSelector: [
            { scheme: 'file', language: 'ron' },
            { scheme: 'untitled', language: 'ron' } // unsaved buffers
        ],
        outputChannelName: 'Autoschematic',
        revealOutputChannelOn: node_1.RevealOutputChannelOn.Never
    };
    const client = new node_1.LanguageClient('autoschematicLsp', 'Autoschematic Language Server', serverOptions, clientOptions);
    context.subscriptions.push(client);
    context.subscriptions.push(vscode.commands.registerCommand('autoschematic.plan', async () => {
        const fileUri = vscode.window.activeTextEditor?.document.uri;
        if (!fileUri) {
            vscode.window.showErrorMessage('No file selected for plan');
            return;
        }
        try {
            const plan_report = await client.sendRequest(node_1.ExecuteCommandRequest.type, {
                command: "plan",
                arguments: [vscode.window.activeTextEditor?.document.fileName]
            }).then(undefined, (error) => {
                vscode.window.showErrorMessage(`Error executing plan command: ${error}`);
            });
            const remoteUri = fileUri.with({ scheme: 'autoschematic-plan', path: fileUri.path + '.json' });
            const diffTitle = `Autoschematic plan: ${fileUri.path.split('/').pop()}`;
            const provider = await vscode.workspace.registerTextDocumentContentProvider('autoschematic-plan', {
                provideTextDocumentContent(uri) {
                    // vscode.window.showErrorMessage("plan: " + plan);
                    return JSON.stringify(plan_report, null, 2);
                }
            });
            context.subscriptions.push(provider);
            // Open the diff editor
            await vscode.window.showTextDocument(remoteUri);
        }
        catch (error) {
            vscode.window.showErrorMessage(`Error comparing with remote: ${error}`);
        }
    }));
    context.subscriptions.push(vscode.commands.registerCommand('autoschematic.apply', async () => {
        const fileUri = vscode.window.activeTextEditor?.document.uri;
        if (!fileUri) {
            vscode.window.showErrorMessage('No file selected for apply');
            return;
        }
        try {
            const apply_report = await client.sendRequest(node_1.ExecuteCommandRequest.type, {
                command: "apply",
                arguments: [vscode.window.activeTextEditor?.document.fileName]
            }).then(undefined, (error) => {
                vscode.window.showErrorMessage(`Error executing apply command: ${error}`);
            });
            const remoteUri = fileUri.with({ scheme: 'autoschematic-remote', path: fileUri.path });
            const diffTitle = `Autoschematic apply: ${fileUri.path.split('/').pop()}`;
            const provider = vscode.workspace.registerTextDocumentContentProvider('autoschematic-remote', {
                provideTextDocumentContent(uri) {
                    return JSON.stringify(apply_report, null, 2);
                    // return plan;
                }
            });
            context.subscriptions.push(provider);
            vscode.window.showTextDocument(remoteUri);
            // // Open the diff editor
            // vscode.commands.executeCommand('vscode.diff',
            // 	fileUri, // Original file URI
            // 	remoteUri, // Modified file URI (virtual)
            // 	diffTitle, // Title for the diff editor
            // 	{ preview: true } // Options
            // );
        }
        catch (error) {
            vscode.window.showErrorMessage(`Error comparing with remote: ${error}`);
        }
    }));
    context.subscriptions.push(vscode.commands.registerCommand('autoschematic.import', () => {
        client.sendRequest(node_1.ExecuteCommandRequest.type, {
            command: "import",
            arguments: []
        }).then(undefined, (error) => {
            vscode.window.showErrorMessage(`Error executing import command: ${error}`);
        });
    }));
    context.subscriptions.push(vscode.commands.registerCommand('autoschematic.relaunch', () => {
        client.sendRequest(node_1.ExecuteCommandRequest.type, {
            command: "relaunch",
            arguments: []
        }).then(undefined, (error) => {
            vscode.window.showErrorMessage(`Error executing relaunch command: ${error}`);
        });
    }));
    await client.start();
}
//# sourceMappingURL=extension.js.map
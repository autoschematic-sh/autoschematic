"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
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
// The module 'vscode' contains the VS Code extensibility API
// Import the module and reference it with the alias vscode in your code below
const vscode = __importStar(require("vscode"));
const node_1 = require("vscode-languageclient/node");
// // This method is called when your extension is activated
// // Your extension is activated the very first time the command is executed
// export function activate(context: vscode.ExtensionContext) {
// 	// Use the console to output diagnostic information (console.log) and errors (console.error)
// 	// This line of code will only be executed once when your extension is activated
// 	console.log('Congratulations, your extension "autoschematic" is now active!');
// 	context.subscriptions.push(disposable);
// }
// // This method is called when your extension is deactivated
// export function deactivate() {}
// const serverOptions: ServerOptions = {
//     run:   { command: "autoschematic-lsp", transport: TransportKind.stdio },
//     debug: { command: "autoschematic-lsp", transport: TransportKind.stdio },
// };
// const clientOptions: LanguageClientOptions = {
//     documentSelector: [{ scheme: "file", language: "ron" }],
// };
// new LanguageClient("autoschematicLsp", "Autoschematic Language Server", serverOptions, clientOptions).start();
async function activate(context) {
    // The command has been defined in the package.json file
    // Now provide the implementation of the command with registerCommand
    // The commandId parameter must match the command field in package.json
    const disposable = vscode.commands.registerCommand('autoschematic.helloWorld', () => {
        // The code you place here will be executed every time your command is executed
        // Display a message box to the user
        vscode.window.showInformationMessage('Hello World from Autoschematic!');
    });
    context.subscriptions.push(disposable);
    // Register the compareWithRemote command that shows a diff view
    context.subscriptions.push(vscode.commands.registerCommand('autoschematic.compareWithRemote', async (fileUri) => {
        // Check if the fileUri is provided (it should be when right-clicking a file)
        if (!fileUri) {
            vscode.window.showErrorMessage('No file selected for comparison');
            return;
        }
        try {
            // Read the content of the selected file
            const document = await vscode.workspace.openTextDocument(fileUri);
            const originalContent = document.getText();
            // Create the remote content (currently a placeholder)
            const remoteContent = await client.sendRequest(node_1.ExecuteCommandRequest.type, {
                command: "get",
                arguments: [fileUri.path]
            });
            // .then(
            // 	(result) => {
            // 		console.log(result);
            // 		// Create a temporary URI for the remote version
            const remoteUri = fileUri.with({ scheme: 'autoschematic-remote', path: fileUri.path + '.remote' });
            // Show the diff editor with the local and remote content
            const diffTitle = `Compare ${fileUri.path.split('/').pop()} with Remote`;
            // 		client.sendRequest(ExecuteCommandRequest.type, {
            // 			command: "get",
            // 			arguments: [fileUri.path]
            // 		}).then(
            // 			(result) => {
            // 				console.log(result);
            // Register a content provider for our custom scheme
            const provider = vscode.workspace.registerTextDocumentContentProvider('autoschematic-remote', {
                provideTextDocumentContent(uri) {
                    return remoteContent;
                }
            });
            // Add the provider to the context subscriptions so it gets disposed when deactivated
            context.subscriptions.push(provider);
            // Open the diff editor
            vscode.commands.executeCommand('vscode.diff', fileUri, // Original file URI
            remoteUri, // Modified file URI (virtual)
            diffTitle, // Title for the diff editor
            { preview: true } // Options
            );
            // 	},
            // 	(error) => {
            // 		vscode.window.showErrorMessage(`Error executing import command: ${error}`);
            // 	}
            // );
        }
        catch (error) {
            vscode.window.showErrorMessage(`Error comparing with remote: ${error}`);
        }
    }));
    // vscode.window.showInformationMessage('Autoschematic extension active');
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
            const plan = await client.sendRequest(node_1.ExecuteCommandRequest.type, {
                command: "plan",
                arguments: [vscode.window.activeTextEditor?.document.fileName]
            }).then(undefined, (error) => {
                vscode.window.showErrorMessage(`Error executing plan command: ${error}`);
            });
            const remoteUri = fileUri.with({ scheme: 'autoschematic-remote', path: fileUri.path + '.remote' });
            const diffTitle = `Autoschematic plan: ${fileUri.path.split('/').pop()}`;
            const provider = vscode.workspace.registerTextDocumentContentProvider('autoschematic-remote', {
                provideTextDocumentContent(uri) {
                    return plan;
                }
            });
            context.subscriptions.push(provider);
            // Open the diff editor
            vscode.commands.executeCommand('vscode.diff', fileUri, // Original file URI
            remoteUri, // Modified file URI (virtual)
            diffTitle, // Title for the diff editor
            { preview: true } // Options
            );
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
            const plan = await client.sendRequest(node_1.ExecuteCommandRequest.type, {
                command: "apply",
                arguments: [vscode.window.activeTextEditor?.document.fileName]
            }).then(undefined, (error) => {
                vscode.window.showErrorMessage(`Error executing apply command: ${error}`);
            });
            const remoteUri = fileUri.with({ scheme: 'autoschematic-remote', path: fileUri.path + '.remote' });
            const diffTitle = `Autoschematic apply: ${fileUri.path.split('/').pop()}`;
            const provider = vscode.workspace.registerTextDocumentContentProvider('autoschematic-remote', {
                provideTextDocumentContent(uri) {
                    return plan;
                }
            });
            context.subscriptions.push(provider);
            // Open the diff editor
            vscode.commands.executeCommand('vscode.diff', fileUri, // Original file URI
            remoteUri, // Modified file URI (virtual)
            diffTitle, // Title for the diff editor
            { preview: true } // Options
            );
        }
        catch (error) {
            vscode.window.showErrorMessage(`Error comparing with remote: ${error}`);
        }
    }));
    context.subscriptions.push(vscode.commands.registerCommand('autoschematic.import', () => {
        // The code you place here will be executed every time your command is executed
        // Send an execute command request to the LSP server
        client.sendRequest(node_1.ExecuteCommandRequest.type, {
            command: "import",
            arguments: []
        }).then(undefined, (error) => {
            vscode.window.showErrorMessage(`Error executing import command: ${error}`);
        });
    }));
    context.subscriptions.push(vscode.commands.registerCommand('autoschematic.relaunch', () => {
        // The code you place here will be executed every time your command is executed
        // Send an execute command request to the LSP server
        client.sendRequest(node_1.ExecuteCommandRequest.type, {
            command: "relaunch",
            arguments: []
        }).then(undefined, (error) => {
            vscode.window.showErrorMessage(`Error executing relaunch command: ${error}`);
        });
    }));
    // Start the server (returns a Promise you may await or ignore)
    await client.start();
}
//# sourceMappingURL=extension.js.map
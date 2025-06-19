import * as vscode from 'vscode';
import * as child_process from 'child_process';
import * as util from 'util';
import { ExecuteCommandRequest, LanguageClient, LanguageClientOptions, RevealOutputChannelOn, ServerOptions, TransportKind } from 'vscode-languageclient/node';

import * as connectorView from './connectorView';

import { activateDecorations } from './decorationProvider';

/**
 * Checks if a command exists in the system PATH
 * @param command The command to check
 * @returns Promise<boolean> True if the command exists, false otherwise
 */
async function commandExists(command: string): Promise<boolean> {
	const exec = util.promisify(child_process.exec);

	try {
		// Different check commands based on platform
		const checkCommand = process.platform === 'win32'
			? `where ${command}`
			: `which ${command}`;

		await exec(checkCommand);
		return true;
	} catch (error) {
		return false;
	}
}

export async function activate(context: vscode.ExtensionContext) {
	// Check if autoschematic-lsp is installed
	const lspExists = await commandExists('autoschematic-lsp');

	if (!lspExists) {
		const action = await vscode.window.showInformationMessage(
			"Autoschematic doesn't appear to be installed yet. Would you like to `cargo install` it?",
			"Yes", "No"
		);

		if (action === "Yes") {
			// Open a terminal and run cargo install
			const terminal = vscode.window.createTerminal('Autoschematic Install');
			terminal.sendText('cargo install --locked autoschematic');
			terminal.show();
			vscode.window.showInformationMessage(
				"Installing Autoschematic. The extension will be ready after installation completes."
			);
			return; // Exit activation until installation completes
		} else {
			// User declined installation
			return; // Exit activation
		}
	}

	vscode.commands.registerCommand('autoschematic.compareWithRemote', async (fileUri) => {
		if (!fileUri) {
			vscode.window.showErrorMessage('No file selected for comparison');
			return;
		}

		try {
			const filterOutput = await client.sendRequest(ExecuteCommandRequest.type, {
				command: "filter",
				arguments: [fileUri.path]
			});

			console.log("filterOutput", filterOutput);
			if (filterOutput != "Resource") {
				vscode.window.showErrorMessage(`Not a resource file for any active connector: ${fileUri.path}`);
				return;
			}

			const remoteContent = await client.sendRequest(ExecuteCommandRequest.type, {
				command: "get",
				arguments: [fileUri.path]
			});

			if (remoteContent === null) {
				vscode.window.showErrorMessage(`Resource doesn't exist remotely: ${fileUri.path}`);
				return;
			}
			const remoteUri = fileUri.with({ scheme: 'autoschematic-remote', path: fileUri.path });

			const diffTitle = `Compare ${fileUri.path.split('/').pop()} with Remote`;
			console.log("remotecontent", remoteContent);

			// Register a content provider for our custom scheme
			const provider = vscode.workspace.registerTextDocumentContentProvider('autoschematic-remote', {
				provideTextDocumentContent(uri: vscode.Uri): string {
					return remoteContent;
				}
			});

			context.subscriptions.push(provider);

			vscode.commands.executeCommand('vscode.diff',
				fileUri, // Original file URI
				remoteUri, // Modified file URI (virtual)
				diffTitle, // Title for the diff editor
				{ preview: true } // Options
			);
		} catch (error) {
			vscode.window.showErrorMessage(`Error comparing with remote: ${error}`);
		}
	});

	const serverOptions: ServerOptions = {
		run: { command: 'autoschematic-lsp', transport: TransportKind.stdio },
		debug: { command: 'autoschematic-lsp', transport: TransportKind.stdio }
	};

	const clientOptions: LanguageClientOptions = {
		documentSelector: [
			{ scheme: 'file', language: 'ron' },
			{ scheme: 'untitled', language: 'ron' } // unsaved buffers
		],
		outputChannelName: 'Autoschematic',
		revealOutputChannelOn: RevealOutputChannelOn.Never
	};

	let client = new LanguageClient(
		'autoschematicLsp',
		'Autoschematic Language Server',
		serverOptions,
		clientOptions
	);

	context.subscriptions.push(client);

	context.subscriptions.push(vscode.commands.registerCommand('autoschematic.getConfigTree', async () => {
		const configTree = await client.sendRequest(ExecuteCommandRequest.type, {
			command: "getConfigTree",
			arguments: []
		});
		return configTree;
	}));

	context.subscriptions.push(vscode.commands.registerCommand('autoschematic.filter', async (fileUri) => {
		const filterResult = await client.sendRequest(ExecuteCommandRequest.type, {
			command: "filter",
			arguments: [fileUri.path]
		});
		return filterResult;
	}));

	context.subscriptions.push(vscode.commands.registerCommand('autoschematic.plan', async () => {
		const fileUri = vscode.window.activeTextEditor?.document.uri;

		if (!fileUri) {
			vscode.window.showErrorMessage('No file selected for plan');
			return;
		}

		try {
			const plan_report = await client.sendRequest(ExecuteCommandRequest.type, {
				command: "plan",
				arguments: [vscode.window.activeTextEditor?.document.fileName]
			}).then(undefined, (error) => {
				vscode.window.showErrorMessage(`Error executing plan command: ${error}`);
			});

			const remoteUri = fileUri.with({ scheme: 'autoschematic-plan', path: fileUri.path + '.json' });

			const diffTitle = `Autoschematic plan: ${fileUri.path.split('/').pop()}`;

			const provider = await vscode.workspace.registerTextDocumentContentProvider('autoschematic-plan', {
				provideTextDocumentContent(uri: vscode.Uri): string {
					// vscode.window.showErrorMessage("plan: " + plan);
					return JSON.stringify(plan_report, null, 2);
				}
			});

			context.subscriptions.push(provider);

			// Open the diff editor
			await vscode.window.showTextDocument(
				remoteUri,
			);
		} catch (error) {
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
			const apply_report = await client.sendRequest(ExecuteCommandRequest.type, {
				command: "apply",
				arguments: [vscode.window.activeTextEditor?.document.fileName]
			}).then(undefined, (error) => {
				vscode.window.showErrorMessage(`Error executing apply command: ${error}`);
			});

			const remoteUri = fileUri.with({ scheme: 'autoschematic-remote', path: fileUri.path });

			const diffTitle = `Autoschematic apply: ${fileUri.path.split('/').pop()}`;

			const provider = vscode.workspace.registerTextDocumentContentProvider('autoschematic-remote', {
				provideTextDocumentContent(uri: vscode.Uri): string {
					return JSON.stringify(apply_report, null, 2);
					// return plan;
				}
			});

			context.subscriptions.push(provider);

			vscode.window.showTextDocument(
				remoteUri,
			);
			// // Open the diff editor
			// vscode.commands.executeCommand('vscode.diff',
			// 	fileUri, // Original file URI
			// 	remoteUri, // Modified file URI (virtual)
			// 	diffTitle, // Title for the diff editor
			// 	{ preview: true } // Options
			// );
		} catch (error) {
			vscode.window.showErrorMessage(`Error comparing with remote: ${error}`);
		}
	}));

	context.subscriptions.push(vscode.commands.registerCommand('autoschematic.import', () => {
		client.sendRequest(ExecuteCommandRequest.type, {
			command: "import",
			arguments: []
		}).then(undefined, (error) => {
			vscode.window.showErrorMessage(`Error executing import command: ${error}`);
		});
	}));

	context.subscriptions.push(vscode.commands.registerCommand('autoschematic.relaunch', () => {
		client = new LanguageClient(
			'autoschematicLsp',
			'Autoschematic Language Server',
			serverOptions,
			clientOptions
		);
		// client.sendRequest(ExecuteCommandRequest.type, {
		// 	command: "relaunch",
		// 	arguments: []
		// }).then(undefined, (error) => {
		// 	vscode.window.showErrorMessage(`Error executing relaunch command: ${error}`);
		// });
	}));

	await client.start();
}

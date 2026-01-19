import * as vscode from 'vscode';
import * as child_process from 'child_process';
import * as util from 'util';
import * as connectorView from './connectorView';
import { ExecuteCommandRequest, LanguageClient, LanguageClientOptions, LSPErrorCodes, RevealOutputChannelOn, ServerOptions, TransportKind } from 'vscode-languageclient/node';

async function handlePullRemoteState(filePath: string, client: LanguageClient): Promise<void> {
	try {
		const action = await vscode.window.showInformationMessage(
			`Pull remote state for ${filePath}?`,
			{ modal: true },
			'Confirm',
			'Cancel'
		);

		if (action !== 'Confirm') {
			return;
		}

		await vscode.window.withProgress({
			location: vscode.ProgressLocation.Notification,
			title: "Pulling remote state...",
			cancellable: false
		}, async (progress) => {
			progress.report({ increment: 0, message: `Fetching remote content for ${filePath}` });

			const remoteContent = await client.sendRequest(ExecuteCommandRequest.type, {
				command: "get_untemplate",
				arguments: [filePath]
			});

			if (remoteContent === null) {
				throw new Error(`Resource doesn't exist remotely: ${filePath}`);
			}

			progress.report({ increment: 50, message: `Writing remote content to ${filePath}` });

			// Write the remote content to the local file
			const fileUri = vscode.Uri.file(filePath);
			const encoder = new TextEncoder();
			await vscode.workspace.fs.writeFile(fileUri, encoder.encode(remoteContent));

			progress.report({ increment: 100, message: "Pull completed" });

			vscode.commands.executeCommand('vscode.open',
				fileUri,
				{ preview: true }
			);
		});

		vscode.window.showInformationMessage(`Successfully pulled remote state for "${filePath}"`);
	} catch (error) {
		vscode.window.showErrorMessage(`Failed to pull remote state: ${error}`);
		throw error;
	}
}


async function handleRenameConfirm(oldPath: string, newPath: string, client: LanguageClient): Promise<void> {
	try {
		await vscode.window.withProgress({
			location: vscode.ProgressLocation.Notification,
			title: "Renaming file...",
			cancellable: false
		}, async (progress) => {
			progress.report({ increment: 0, message: `Renaming ${oldPath} to ${newPath}` });

			const result = await client.sendRequest(ExecuteCommandRequest.type, {
				command: "rename",
				arguments: [oldPath, newPath]
			});

			console.log(result);

			progress.report({ increment: 100, message: "Rename completed" });
		});

		vscode.window.showInformationMessage(`Successfully renamed "${oldPath}" to "${newPath}"`);
	} catch (error) {
		vscode.window.showErrorMessage(`Failed to rename file: ${error}`);
		throw error;
	}
}

/**
 * Checks if a command exists in the system PATH
 * @param command The command to check
 * @returns Promise<boolean> True if the command exists, false otherwise
 */
async function commandExists(command: string): Promise<boolean> {
	const exec = util.promisify(child_process.exec);

	try {
		const checkCommand = process.platform === 'win32'
			? `where ${command}`
			: `which ${command}`;

		await exec(checkCommand);
		return true;
	} catch (error) {
		return false;
	}
}

let clientRef: { current: LanguageClient | null } = { current: null };

export async function activate(context: vscode.ExtensionContext) {
	// Check if autoschematic-lsp is installed
	const lspExists = await commandExists('autoschematic-lsp');

	if (!lspExists) {
		const action = await vscode.window.showInformationMessage(
			"The Autoschematic language server doesn't appear to be installed yet. Would you like to `cargo install` it?",
			"Yes", "No"
		);

		if (action === "Yes") {
			const terminal = vscode.window.createTerminal('Autoschematic Install');
			terminal.sendText('cargo install --locked autoschematic-lsp');
			terminal.show();
			vscode.window.showInformationMessage(
				"Installing autoschematic-lsp. After installation completes, restart the extension with Ctrl-P or close and re-open the window."
			);
			return;
		} else {
			return;
		}
	}

	// vscode.commands.registerCommand('autoschematic.rename', async (fileUri) => {
	// 	if (!fileUri) {
	// 		vscode.window.showErrorMessage('No file selected for rename');
	// 		return;
	// 	}

	// 	const currentPath = fileUri.path;

	// 	const newPath = await vscode.window.showInputBox({
	// 		title: 'Rename Resource',
	// 		prompt: 'Enter the new resource path',
	// 		value: currentPath,
	// 		valueSelection: [currentPath.lastIndexOf('/') + 1, currentPath.lastIndexOf('.')],
	// 		validateInput: (value: string) => {
	// 			if (!value || value.trim() === '') {
	// 				return 'Path cannot be empty';
	// 			}
	// 			if (value === currentPath) {
	// 				return 'New path must be different from current path';
	// 			}
	// 			return null;
	// 		}
	// 	});

	// 	if (newPath === undefined) {
	// 		// User cancelled
	// 		return;
	// 	}

	// 	try {
	// 		const action = await vscode.window.showInformationMessage(
	// 			`Rename resource \n   "${currentPath}"\nto "${newPath}"?`,
	// 			{ modal: true },
	// 			'Confirm',
	// 			'Cancel'
	// 		);

	// 		if (action === 'Confirm') {
	// 			await handleRenameConfirm(currentPath, newPath, client);
	// 		}
	// 	} catch (error) {
	// 		console.log(error)
	// 		vscode.window.showErrorMessage(`Error during rename: ${error}`);
	// 	}
	// });

	context.subscriptions.push(vscode.commands.registerCommand('autoschematic.editOutputs', async (fileUri) => {
		if (!fileUri) {
			vscode.window.showErrorMessage('No file selected for edit');
			return;
		}

		const wsFolder = vscode.workspace.getWorkspaceFolder(fileUri);
		if (!wsFolder) {
			vscode.window.showErrorMessage('File is not inside a workspace folder.');
			return;
		}

		const rel = vscode.workspace.asRelativePath(fileUri, false);

		const targetUri = vscode.Uri.joinPath(
			wsFolder.uri,
			'.autoschematic',
			rel + '.out.ron'
		);

		const doc = await vscode.workspace.openTextDocument(targetUri);
		await vscode.window.showTextDocument(doc);
	}));

	context.subscriptions.push(vscode.commands.registerCommand('autoschematic.compareWithRemote', async (fileUri) => {
		if (!fileUri) {
			vscode.window.showErrorMessage('No file selected for comparison');
			return;
		}

		if (!clientRef.current) {
			vscode.window.showErrorMessage('Language server not running');
			return;
		}

		try {
			const filterResponse = await clientRef.current.sendRequest(ExecuteCommandRequest.type, {
				command: "filter",
				arguments: [fileUri.path]
			});
			

			if (filterResponse === null) {
				vscode.window.showErrorMessage(`No Autoschematic config is present.`);
				return;
			}

			if (!filterResponse.includes("Resource")) {
				vscode.window.showErrorMessage(`Not a resource file for any active connector: ${fileUri.path}`);
				return;
			}

			vscode.window.withProgress({
				location: vscode.ProgressLocation.Notification,
				title: "Fetching remote for comparison...",
				cancellable: true
			}, async (progress, token) => {
				let canceled = false;
				token.onCancellationRequested(() => {
					canceled = true;
				});

				progress.report({ increment: 0 });

				try {
					const remoteContent = await clientRef.current!.sendRequest(ExecuteCommandRequest.type, {
						command: "get_untemplate",
						arguments: [fileUri.path]
					});

					if (canceled) {
						return;
					}

					if (remoteContent === null) {
						vscode.window.showErrorMessage(`Resource doesn't exist remotely: ${fileUri.path}`);
						return;
					}
					const remoteUri = fileUri.with({ scheme: 'autoschematic-remote', path: fileUri.path });

					const diffTitle = `Compare ${fileUri.path.split('/').pop()} with Remote`;

					const provider = vscode.workspace.registerTextDocumentContentProvider('autoschematic-remote', {
						provideTextDocumentContent(uri: vscode.Uri): string {
							return remoteContent;
						}
					});

					context.subscriptions.push(provider);

					if (canceled) {
						return;
					}

					// Open the local file first to ensure VSCode knows it exists
					// await vscode.workspace.openTextDocument(fileUri);

					vscode.commands.executeCommand('vscode.diff',
						fileUri,
						remoteUri,
						diffTitle,
						{ preview: false }
					);
				} catch (e) {
					vscode.window.showErrorMessage(`Error comparing with remote: ${fileUri.path}: ${e}`);
				}
			});
		} catch (error) {
			vscode.window.showErrorMessage(`Error comparing with remote: ${error}`);
		}
	}));

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

	clientRef.current = client;

	context.subscriptions.push(client);

	context.subscriptions.push(vscode.commands.registerCommand('autoschematic.getConfigTree', async () => {
		if (!clientRef.current) {
			vscode.window.showErrorMessage('Language server not running');
			return;
		}
		const configTree = await clientRef.current.sendRequest(ExecuteCommandRequest.type, {
			command: "getConfigTree",
			arguments: []
		});
		return configTree;
	}));

	context.subscriptions.push(vscode.commands.registerCommand('autoschematic.filter', async (fileUri) => {
		if (!clientRef.current) {
			vscode.window.showErrorMessage('Language server not running');
			return;
		}
		const filterResult = await clientRef.current.sendRequest(ExecuteCommandRequest.type, {
			command: "filter",
			arguments: [fileUri.path]
		});
		return filterResult;
	}));

	context.subscriptions.push(vscode.commands.registerCommand('autoschematic.pullRemoteState', async (uri) => {
		if (!clientRef.current) {
			vscode.window.showErrorMessage('Language server not running');
			return;
		}

		// Extract the file path from the URI
		// The URI will be the autoschematic-remote URI, but we need the original file path
		let filePath: string;

		if (uri && uri.scheme === 'autoschematic-remote') {
			// Use the path from the remote URI (which should be the original file path)
			filePath = uri.path;
		} else {
			// Fallback: try to get the file path from the active editor
			const activeEditor = vscode.window.activeTextEditor;
			if (activeEditor && activeEditor.document.uri.scheme === 'file') {
				filePath = activeEditor.document.uri.path;
			} else {
				vscode.window.showErrorMessage('Unable to determine file path for pull operation');
				return;
			}
		}

		try {
			await handlePullRemoteState(filePath, clientRef.current);
		} catch (error) {
			console.log(error);
			vscode.window.showErrorMessage(`Error during pull remote state: ${error}`);
		}
	}));

	context.subscriptions.push(vscode.commands.registerCommand('autoschematic.relaunch', async () => {
		try {
			// Stop the old client if it exists
			if (clientRef.current) {
				await clientRef.current.stop();
			}

			// Create a new client with the same options
			const newClient = new LanguageClient(
				'autoschematicLsp',
				'Autoschematic Language Server',
				serverOptions,
				clientOptions
			);

			// Update the reference so all commands use the new client
			clientRef.current = newClient;
			context.subscriptions.push(newClient);

			// Start the new client
			await newClient.start();
			vscode.window.showInformationMessage('Restarted Autoschematic language server');

			// Reactivate the connector view with the new client
			connectorView.activate(context, newClient);
		} catch (error) {
			vscode.window.showErrorMessage(`Error relaunching Autoschematic Language Server: ${error}`);
		}
	}));

	client.start()
		.then(undefined, (error) => {
			vscode.window.showErrorMessage(`Error starting Autoschematic Language Server: ${error}`);
		});


	connectorView.activate(context, client);
}

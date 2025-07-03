import * as vscode from 'vscode';
import * as child_process from 'child_process';
import * as util from 'util';
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
				"Installing autoschematic-lsp. The extension will be ready after installation completes."
			);
			return;
		} else {
			return;
		}
	}

	vscode.commands.registerCommand('autoschematic.rename', async (fileUri) => {
		if (!fileUri) {
			vscode.window.showErrorMessage('No file selected for rename');
			return;
		}

		const currentPath = fileUri.path;

		const newPath = await vscode.window.showInputBox({
			title: 'Rename Resource',
			prompt: 'Enter the new resource path',
			value: currentPath,
			valueSelection: [currentPath.lastIndexOf('/') + 1, currentPath.lastIndexOf('.')],
			validateInput: (value: string) => {
				if (!value || value.trim() === '') {
					return 'Path cannot be empty';
				}
				if (value === currentPath) {
					return 'New path must be different from current path';
				}
				return null;
			}
		});

		if (newPath === undefined) {
			// User cancelled
			return;
		}

		try {
			const action = await vscode.window.showInformationMessage(
				`Rename resource \n   "${currentPath}"\nto "${newPath}"?`,
				{ modal: true },
				'Confirm',
				'Cancel'
			);

			if (action === 'Confirm') {
				await handleRenameConfirm(currentPath, newPath, client);
			}
		} catch (error) {
			console.log(error)
			vscode.window.showErrorMessage(`Error during rename: ${error}`);
		}
	});


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
					const remoteContent = await client.sendRequest(ExecuteCommandRequest.type, {
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

					vscode.commands.executeCommand('vscode.diff',
						fileUri,
						remoteUri,
						diffTitle,
						{ preview: true }
					);
				} catch (e) {
					vscode.window.showErrorMessage(`Error comparing with remote: ${fileUri.path}: ${e}`);
				}
			});
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

	// context.subscriptions.push(vscode.commands.registerCommand('autoschematic.plan', async () => {
	// 	const fileUri = vscode.window.activeTextEditor?.document.uri;

	// 	if (!fileUri) {
	// 		vscode.window.showErrorMessage('No file selected for plan');
	// 		return;
	// 	}

	// 	try {
	// 		const plan_report = await client.sendRequest(ExecuteCommandRequest.type, {
	// 			command: "plan",
	// 			arguments: [vscode.window.activeTextEditor?.document.fileName]
	// 		}).then(undefined, (error) => {
	// 			vscode.window.showErrorMessage(`Error executing plan command: ${error}`);
	// 		});

	// 		const remoteUri = fileUri.with({ scheme: 'autoschematic-plan', path: fileUri.path + '.json' });

	// 		const diffTitle = `Autoschematic plan: ${fileUri.path.split('/').pop()}`;

	// 		const provider = await vscode.workspace.registerTextDocumentContentProvider('autoschematic-plan', {
	// 			provideTextDocumentContent(uri: vscode.Uri): string {
	// 				// vscode.window.showErrorMessage("plan: " + plan);
	// 				return JSON.stringify(plan_report, null, 2);
	// 			}
	// 		});

	// 		context.subscriptions.push(provider);

	// 		// Open the diff editor
	// 		await vscode.window.showTextDocument(
	// 			remoteUri,
	// 		);
	// 	} catch (error) {
	// 		vscode.window.showErrorMessage(`Error comparing with remote: ${error}`);
	// 	}
	// }));

	// context.subscriptions.push(vscode.commands.registerCommand('autoschematic.apply', async () => {
	// 	const fileUri = vscode.window.activeTextEditor?.document.uri;

	// 	if (!fileUri) {
	// 		vscode.window.showErrorMessage('No file selected for apply');
	// 		return;
	// 	}

	// 	try {
	// 		const apply_report = await client.sendRequest(ExecuteCommandRequest.type, {
	// 			command: "apply",
	// 			arguments: [vscode.window.activeTextEditor?.document.fileName]
	// 		}).then(undefined, (error) => {
	// 			vscode.window.showErrorMessage(`Error executing apply command: ${error}`);
	// 		});

	// 		const remoteUri = fileUri.with({ scheme: 'autoschematic-remote', path: fileUri.path });

	// 		const diffTitle = `Autoschematic apply: ${fileUri.path.split('/').pop()}`;

	// 		const provider = vscode.workspace.registerTextDocumentContentProvider('autoschematic-remote', {
	// 			provideTextDocumentContent(uri: vscode.Uri): string {
	// 				return JSON.stringify(apply_report, null, 2);
	// 				// return plan;
	// 			}
	// 		});

	// 		context.subscriptions.push(provider);

	// 		vscode.window.showTextDocument(
	// 			remoteUri,
	// 		);
	// 		// // Open the diff editor
	// 		// vscode.commands.executeCommand('vscode.diff',
	// 		// 	fileUri, // Original file URI
	// 		// 	remoteUri, // Modified file URI (virtual)
	// 		// 	diffTitle, // Title for the diff editor
	// 		// 	{ preview: true } // Options
	// 		// );
	// 	} catch (error) {
	// 		vscode.window.showErrorMessage(`Error comparing with remote: ${error}`);
	// 	}
	// }));

	context.subscriptions.push(vscode.commands.registerCommand('autoschematic.pullRemoteState', async (uri) => {
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
			await handlePullRemoteState(filePath, client);
		} catch (error) {
			console.log(error);
			vscode.window.showErrorMessage(`Error during pull remote state: ${error}`);
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

	context.subscriptions.push(vscode.commands.registerCommand('autoschematic.relaunch', async () => {
		client.restart()
			.then(undefined, (error) => {
				vscode.window.showErrorMessage(`Error restarting Autoschematic Language Server: ${error}`);
			});
		// client = new LanguageClient(
		// 	'autoschematicLsp',
		// 	'Autoschematic Language Server',
		// 	serverOptions,
		// 	clientOptions
		// );
		// client.sendRequest(ExecuteCommandRequest.type, {
		// 	command: "relaunch",
		// 	arguments: []
		// }).then(undefined, (error) => {
		// 	vscode.window.showErrorMessage(`Error executing relaunch command: ${error}`);
		// });
	}));

	client.start()
		.then(undefined, (error) => {
			vscode.window.showErrorMessage(`Error starting Autoschematic Language Server: ${error}`);
		});
}

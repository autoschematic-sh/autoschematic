import * as vscode from 'vscode';

export function activateDecorations(ctx: vscode.ExtensionContext) {
    const provider = new WidgetDecorationProvider();
    ctx.subscriptions.push(
        vscode.window.registerFileDecorationProvider(provider)  // <-- key line
    );
}

class WidgetDecorationProvider implements vscode.FileDecorationProvider {
    private _emitter = new vscode.EventEmitter<vscode.Uri | vscode.Uri[]>();
    readonly onDidChangeFileDecorations = this._emitter.event;

    provideFileDecoration(
        uri: vscode.Uri
    ): vscode.ProviderResult<vscode.FileDecoration> {

        let filterResponse = vscode.commands.executeCommand('autoschematic.filter');
        if (uri.path.endsWith('.ron')) {
            return {
                badge: 'ℝ',
                // OR   themeIcon: new vscode.ThemeIcon('gear'),
                // tooltip: 'Generated file',
                // color: new vscode.ThemeColor(
                //     'gitDecoration.modifiedResourceForeground'
                // )
            };
        }
        'ℂ';
        return undefined;          // no widget for this file
    }

    /** call this when your state changes */
    refresh(uri: vscode.Uri) {
        this._emitter.fire(uri);
    }
}
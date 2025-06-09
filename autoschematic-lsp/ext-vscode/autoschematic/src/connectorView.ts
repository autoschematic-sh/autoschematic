import * as vscode from 'vscode';
import * as path from 'path';

type Sev = 'error' | 'warn' | 'ok';

class Prefix {
    constructor(
        public readonly label: string,
        public readonly sev: Sev,
        public readonly children: Prefix[] = []
    ) { }
}

class StatusProvider implements vscode.TreeDataProvider<Prefix> {
    private _onDidChangeTreeData = new vscode.EventEmitter<Prefix | undefined>();
    readonly onDidChangeTreeData = this._onDidChangeTreeData.event;

    getTreeItem(e: Prefix): vscode.TreeItem {
        const item = new vscode.TreeItem(
            e.label,
            e.children.length ? vscode.TreeItemCollapsibleState.Collapsed
                : vscode.TreeItemCollapsibleState.None
        );
        item.iconPath = iconFor(e.sev);
        return item;
    }

    getChildren(e?: Prefix): Prefix[] {
        return e ? e.children : this.root;
    }

    // replace with real data + refresh() when it changes
    root: Prefix[] = [
        new Prefix('Top-level red', 'error', [
            new Prefix('Nested green', 'ok'),
            new Prefix('Nested orange', 'warn')
        ])
    ];
}

function iconFor(sev: Sev): vscode.ThemeIcon {
    // VS Code ≥ 1.75 can tint codicons
    // if ((vscode.ThemeIcon as any).hasOwnProperty('color')) {
    const color = sev === 'error' ? 'errorForeground' :
        sev === 'warn' ? 'warningForeground' :
            'charts.green';
    return new vscode.ThemeIcon('circle-filled', new vscode.ThemeColor(color));
    // }

    // Fallback SVGs (media/error.svg, warn.svg, ok.svg)
    // const media = path.join(__dirname, 'media');
    // return {
    //     light: path.join(media, `${sev}.svg`),
    //     dark: path.join(media, `${sev}.svg`)
    // };
}

export function activate(ctx: vscode.ExtensionContext) {
    const provider = new StatusProvider();
    ctx.subscriptions.push(
        vscode.window.registerTreeDataProvider('autoschematicStatusView', provider)
    );
}
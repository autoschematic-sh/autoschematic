import * as vscode from 'vscode';
import * as path from 'path';
import { decimal, ExecuteCommandRequest, LanguageClient } from 'vscode-languageclient/node';

type Sev = 'error' | 'warn' | 'ok';

function formatBytes(bytes: number, decimals = 2) {
    if (!+bytes) return '0 Bytes'

    const k = 1024
    const dm = decimals < 0 ? 0 : decimals
    const sizes = ['Bytes', 'KiB', 'MiB', 'GiB', 'TiB', 'PiB', 'EiB', 'ZiB', 'YiB']

    const i = Math.floor(Math.log(bytes) / Math.log(k))

    return `${parseFloat((bytes / Math.pow(k, i)).toFixed(dm))} ${sizes[i]}`
}

class Prefix {
    constructor(
        public readonly label: string,
        public readonly cpu: string = '',
        public readonly ram: string = '',
        public readonly sev: Sev = 'ok',
        public readonly children: Connector[]
    ) { }
}

class Connector {
    constructor(
        public readonly label: string,
        public readonly cpu: string,
        public readonly ram: string,
        public readonly sev: Sev,
        public readonly children: Connector[] = []
    ) { }
}

class StatusProvider implements vscode.TreeDataProvider<Prefix | Connector> {
    private _onDidChangeTreeData = new vscode.EventEmitter<Prefix | Connector | undefined | void>();
    readonly onDidChangeTreeData = this._onDidChangeTreeData.event;
    private client: LanguageClient;

    constructor(client: LanguageClient) {
        this.client = client;
        setInterval(() => { this.refresh() }, 1000);
    }

    getTreeItem(e: Prefix | Connector): vscode.TreeItem {
        if (e.children.length) {
            const item = new vscode.TreeItem(
                e.label,
                vscode.TreeItemCollapsibleState.Collapsed
            );
            return item;
        } else {
            const item = new vscode.TreeItem(
                `${e.label} ${e.cpu} ${e.ram}`,
                vscode.TreeItemCollapsibleState.None
            );
            item.iconPath = iconFor(e.sev);
            return item;
        }
    }


    // getTreeItem(e: Connector): vscode.TreeItem {
    //     const item = new vscode.TreeItem(
    //         e.label,
    //     );
    //     // item.iconPath = iconFor(e.sev);
    //     return item;
    // }

    getChildren(e?: Prefix | Connector): Prefix[] | Connector[] {
        return e ? e.children : this.root;
    }

    async refresh() {
        const top = await this.client.sendRequest(ExecuteCommandRequest.type, {
            command: "top",
            arguments: []
        });

        this.root = [];

        for (const prefix in top) {
            let connectors: Connector[] = [];

            for (const connector in top[prefix]) {
                if ('Alive' in top[prefix][connector]) {
                    connectors.push(new Connector(
                        connector,
                        `${top[prefix][connector]['Alive']['cpu_usage'].toFixed(2)}%`,
                        formatBytes(top[prefix][connector]['Alive']['memory']),
                        'ok',
                        []
                    ));
                } else if ('Dead' in top[prefix][connector]) {
                    connectors.push(new Connector(
                        connector,
                        "~",
                        "~",
                        'error',
                        []
                    ));
                }
            }

            this.root.push(new Prefix(
                prefix,
                "", "", 'ok', connectors,
            ));
        }

        this._onDidChangeTreeData.fire();
    }

    // replace with real data + refresh() when it changes
    root: Prefix[] = [
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

export function activate(ctx: vscode.ExtensionContext, client: LanguageClient) {
    const provider = new StatusProvider(client);
    ctx.subscriptions.push(
        vscode.window.registerTreeDataProvider('connector-summary', provider)
    );
}
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
    private refreshInterval: NodeJS.Timeout | undefined;

    constructor(client: LanguageClient) {
        this.client = client;
        this.refreshInterval = setInterval(() => { this.refresh() }, 1000);
    }

    dispose() {
        if (this.refreshInterval) {
            clearInterval(this.refreshInterval);
            this.refreshInterval = undefined;
        }
        this._onDidChangeTreeData.dispose();
    }

    updateClient(client: LanguageClient) {
        this.client = client;
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
        let top;
        try {
            top = await this.client.sendRequest(ExecuteCommandRequest.type, {
                command: "top",
                arguments: []
            });
        } catch (e) {
            // Client not ready yet (e.g., during restart) - clear and show empty state
            this.root = [];
            this._onDidChangeTreeData.fire();
            return;
        }

        this.root = [];

        for (const prefix in top) {
            let connectors: Connector[] = [];

            for (const connector in top[prefix]) {
                let status = top[prefix][connector];
                let handle_status = status['handle_status'];

                if ('Alive' in handle_status) {
                    connectors.push(new Connector(
                        connector,
                        `${handle_status['Alive']['cpu_usage'].toFixed(2)}%`,
                        formatBytes(handle_status['Alive']['memory']),
                        'ok',
                        []
                    ));
                } else if ('Dead' in handle_status) {
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

let providerRef: StatusProvider | undefined;

export function activate(ctx: vscode.ExtensionContext, client: LanguageClient) {
    if (providerRef) {
        // Reuse existing provider, just update the client reference
        providerRef.updateClient(client);
    } else {
        // first, create a new provider and register it
        providerRef = new StatusProvider(client);
        const disposable = vscode.window.registerTreeDataProvider('connector-summary', providerRef);
        ctx.subscriptions.push(disposable);

        // dispose the provider when the extension deactivates
        ctx.subscriptions.push({
            dispose: () => {
                if (providerRef) {
                    providerRef.dispose();
                    providerRef = undefined;
                }
            }
        });
    }
}
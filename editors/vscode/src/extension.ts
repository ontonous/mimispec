import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import { spawn } from 'child_process';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
} from 'vscode-languageclient/node';

interface JsonError {
    code: string;
    line: number;
    col: number;
    message: string;
}

interface JsonResult {
    path: string;
    success: boolean;
    partial?: boolean;
    status?: 'complete' | 'partial';
    errors: JsonError[];
}

interface JsonOutput {
    schema_version?: string;
    results: JsonResult[];
}

interface ByteSpan { start: number; end: number; }
interface QueueItemWire {
    slot: number;
    state: string;
    anchor: string;
    header: string;
    span: ByteSpan;
}
interface QueueScopeWire {
    scope_path: string[];
    header: string;
    node?: number;
    span?: ByteSpan;
    decision_count: number;
    delegation_count: number;
    children: QueueScopeWire[];
    items: QueueItemWire[];
}
interface DocumentSnapshotWire {
    schema_version: string;
    queue_tree?: { root: QueueScopeWire };
}

const PARSE_SCHEMA = 'mimispec.parse/0.3';
const LANG = 'mimispec';
const CFG = 'mimispec';

let client: LanguageClient | undefined;
let diags: vscode.DiagnosticCollection | undefined;
let chan: vscode.OutputChannel;
let legacyMode = false;
let queueProvider: QueueTreeProvider | undefined;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
    chan = vscode.window.createOutputChannel('MimiSpec');
    context.subscriptions.push(chan);

    const bin = await findBinary(['target/release/mimispec', 'target/debug/mimispec']);
    if (!bin) {
        chan.appendLine('MimiSpec binary not found. Set "mimispec.binaryPath" or build the Rust parser.');
        return;
    }

    if (await supportsLanguageServer(bin)) {
        await startLanguageClient(context, bin);
        queueProvider = new QueueTreeProvider();
        context.subscriptions.push(vscode.window.registerTreeDataProvider('mimispecQueues', queueProvider));
        context.subscriptions.push(vscode.commands.registerCommand('mimispec.refreshQueues', () => queueProvider?.refresh()));
        context.subscriptions.push(vscode.window.onDidChangeActiveTextEditor(() => queueProvider?.refresh()));
        context.subscriptions.push(vscode.workspace.onDidChangeTextDocument((event) => {
            if (event.document.languageId === LANG) queueProvider?.refresh();
        }));
    } else {
        legacyMode = true;
        chan.appendLine('MimiSpec 0.3 LSP is unavailable; using the reduced 0.2.1 file-validation fallback.');
        initLegacyValidation(context, bin);
    }

    context.subscriptions.push(vscode.commands.registerCommand('mimispec.validateFile', async () => {
        const editor = vscode.window.activeTextEditor;
        if (!editor || editor.document.languageId !== LANG) {
            void vscode.window.showInformationMessage('No active MimiSpec file to validate.');
            return;
        }
        if (legacyMode) {
            await validateLegacy(bin, editor.document);
        } else {
            void vscode.window.showInformationMessage('MimiSpec live language-server validation is active.');
        }
    }));
}

type QueueTreeEntry =
    | { kind: 'scope'; value: QueueScopeWire }
    | { kind: 'item'; value: QueueItemWire };

class QueueTreeProvider implements vscode.TreeDataProvider<QueueTreeEntry> {
    private readonly changed = new vscode.EventEmitter<QueueTreeEntry | undefined | void>();
    readonly onDidChangeTreeData = this.changed.event;
    private root: QueueScopeWire | undefined;
    private uri: vscode.Uri | undefined;

    refresh(): void {
        this.root = undefined;
        this.changed.fire();
    }

    getTreeItem(entry: QueueTreeEntry): vscode.TreeItem {
        if (entry.kind === 'scope') {
            const scope = entry.value;
            const item = new vscode.TreeItem(
                scope.header,
                scope.children.length || scope.items.length
                    ? vscode.TreeItemCollapsibleState.Collapsed
                    : vscode.TreeItemCollapsibleState.None,
            );
            item.description = `${scope.decision_count} decision · ${scope.delegation_count} delegation`;
            item.tooltip = scope.scope_path.join(' / ') || '<document>';
            return item;
        }
        const queue = entry.value;
        const item = new vscode.TreeItem(`[${queue.state || 'none'}] ${queue.anchor}`);
        item.description = queue.header.trim();
        item.tooltip = `slot ${queue.slot}: ${queue.header.trim()}`;
        if (this.uri) {
            item.command = {
                command: 'vscode.open',
                title: 'Go to MimiSpec queue item',
                arguments: [this.uri, { selection: rangeFromByteSpan(this.uri, queue.span) }],
            };
        }
        return item;
    }

    async getChildren(entry?: QueueTreeEntry): Promise<QueueTreeEntry[]> {
        if (!entry) {
            await this.load();
            return this.root ? [{ kind: 'scope', value: this.root }] : [];
        }
        if (entry.kind === 'item') return [];
        return [
            ...entry.value.children.map((value): QueueTreeEntry => ({ kind: 'scope', value })),
            ...entry.value.items.map((value): QueueTreeEntry => ({ kind: 'item', value })),
        ].sort((left, right) => queueEntryStart(left) - queueEntryStart(right));
    }

    private async load(): Promise<void> {
        if (this.root || !client) return;
        const document = vscode.window.activeTextEditor?.document;
        if (!document || document.languageId !== LANG) return;
        this.uri = document.uri;
        try {
            const snapshot = await client.sendRequest<DocumentSnapshotWire>(
                'mimispec/documentSnapshot',
                { textDocument: { uri: document.uri.toString() } },
            );
            if (snapshot.schema_version === 'mimispec.ls/0.3') this.root = snapshot.queue_tree?.root;
        } catch (error) {
            chan.appendLine(`Unable to refresh MimiSpec queues: ${String(error)}`);
        }
    }
}

function queueEntryStart(entry: QueueTreeEntry): number {
    return entry.kind === 'scope'
        ? (entry.value.span?.start ?? Number.MAX_SAFE_INTEGER)
        : entry.value.span.start;
}

function rangeFromByteSpan(uri: vscode.Uri, span: ByteSpan): vscode.Range {
    const document = vscode.workspace.textDocuments.find((candidate) => candidate.uri.toString() === uri.toString());
    if (!document) return new vscode.Range(0, 0, 0, 0);
    const bytes = Buffer.from(document.getText(), 'utf8');
    const start = bytes.subarray(0, Math.min(span.start, bytes.length)).toString('utf8').length;
    const end = bytes.subarray(0, Math.min(span.end, bytes.length)).toString('utf8').length;
    return new vscode.Range(document.positionAt(start), document.positionAt(end));
}

async function startLanguageClient(context: vscode.ExtensionContext, bin: string): Promise<void> {
    const collaborationMode = getCfg<'advisory' | 'strict'>(CFG, 'collaborationMode', 'advisory');
    const serverOptions: ServerOptions = {
        command: bin,
        args: ['lsp', '--stdio'],
    };
    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: 'file', language: LANG }, { scheme: 'untitled', language: LANG }],
        synchronize: { configurationSection: CFG },
        initializationOptions: { collaborationMode },
        outputChannel: chan,
    };
    client = new LanguageClient('mimispec', 'MimiSpec Language Server', serverOptions, clientOptions);
    context.subscriptions.push(client);
    await client.start();
    chan.appendLine(`MimiSpec 0.3 language server started in ${collaborationMode} mode.`);
}

function initLegacyValidation(context: vscode.ExtensionContext, bin: string): void {
    diags = vscode.languages.createDiagnosticCollection(LANG);
    context.subscriptions.push(diags);
    context.subscriptions.push(vscode.workspace.onDidSaveTextDocument((document) => {
        if (document.languageId === LANG && getCfg<boolean>(CFG, 'validateOnSave', true)) {
            void validateLegacy(bin, document);
        }
    }));
    context.subscriptions.push(vscode.workspace.onDidOpenTextDocument((document) => {
        if (document.languageId === LANG && getCfg<boolean>(CFG, 'validateOnOpen', true)) {
            void validateLegacy(bin, document);
        }
    }));
    context.subscriptions.push(vscode.workspace.onDidCloseTextDocument((document) => {
        if (document.languageId === LANG) diags?.delete(document.uri);
    }));
    for (const document of vscode.workspace.textDocuments) {
        if (document.languageId === LANG && getCfg<boolean>(CFG, 'validateOnOpen', true)) {
            void validateLegacy(bin, document);
        }
    }
}

async function validateLegacy(bin: string, document: vscode.TextDocument): Promise<void> {
    let output: string;
    try {
        output = await runBinary(bin, ['--json', '-'], document.getText());
    } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        chan.appendLine(`Legacy validation failed: ${message}`);
        return;
    }
    let parsed: JsonOutput;
    try {
        parsed = JSON.parse(output) as JsonOutput;
    } catch {
        chan.appendLine(`Unexpected parser output: ${output}`);
        return;
    }
    if (parsed.schema_version && parsed.schema_version !== PARSE_SCHEMA) {
        void vscode.window.showErrorMessage(`Unsupported MimiSpec parse schema: ${parsed.schema_version}`);
        return;
    }
    const result = parsed.results[0];
    if (!result) {
        diags?.delete(document.uri);
        return;
    }
    const diagnostics = result.errors.map((error) => {
        const line = Math.max(1, error.line) - 1;
        const column = Math.max(1, error.col) - 1;
        const diagnostic = new vscode.Diagnostic(
            new vscode.Range(line, column, line, column + 1),
            `[${error.code}] ${error.message}`,
            vscode.DiagnosticSeverity.Error,
        );
        diagnostic.source = LANG;
        return diagnostic;
    });
    diags?.set(document.uri, diagnostics);
    const partial = result.partial ?? (result.status ? result.status === 'partial' : !result.success);
    chan.appendLine(partial
        ? `${diagnostics.length} error(s) in ${path.basename(document.uri.fsPath)}`
        : `No errors in ${path.basename(document.uri.fsPath)}`);
}

async function supportsLanguageServer(bin: string): Promise<boolean> {
    try {
        const output = await runBinary(bin, ['lsp', '--help'], '');
        return output.includes('long-lived MimiSpec 0.3 language server');
    } catch {
        return false;
    }
}

function getCfg<T>(section: string, key: string, defaultValue: T): T {
    return vscode.workspace.getConfiguration(section).get<T>(key, defaultValue);
}

async function findBinary(candidates: string[]): Promise<string | undefined> {
    const configured = getCfg<string | null>(CFG, 'binaryPath', null);
    if (configured && fs.existsSync(configured)) return configured;
    for (const folder of vscode.workspace.workspaceFolders ?? []) {
        for (const candidate of candidates) {
            const full = path.join(folder.uri.fsPath, candidate);
            if (fs.existsSync(full)) return full;
        }
    }
    return undefined;
}

function runBinary(bin: string, args: string[], input: string): Promise<string> {
    return new Promise((resolve, reject) => {
        const child = spawn(bin, args, { stdio: ['pipe', 'pipe', 'pipe'] });
        let stdout = '';
        let stderr = '';
        child.stdout.on('data', (data: Buffer) => { stdout += data.toString('utf-8'); });
        child.stderr.on('data', (data: Buffer) => { stderr += data.toString('utf-8'); });
        child.on('error', reject);
        child.on('close', (code) => {
            if (code !== 0 && code !== 1) reject(new Error(stderr || `${bin} exited with code ${code}`));
            else resolve(stdout);
        });
        child.stdin.end(input, 'utf-8');
    });
}

export async function deactivate(): Promise<void> {
    if (client) await client.stop();
    diags?.dispose();
    chan?.dispose();
}

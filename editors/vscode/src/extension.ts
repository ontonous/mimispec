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

const PARSE_SCHEMA = 'mimispec.parse/0.3';
const LANG = 'mimispec';
const CFG = 'mimispec';

let client: LanguageClient | undefined;
let diags: vscode.DiagnosticCollection | undefined;
let chan: vscode.OutputChannel;
let legacyMode = false;

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

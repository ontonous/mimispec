import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import { spawn, ChildProcess } from 'child_process';
import {
    LanguageClient,
    LanguageClientOptions,
    ServerOptions,
    State as LspState,
} from 'vscode-languageclient/node';

// ---------------------------------------------------------------------------
// MimiSpec (CLI-based validation)
// ---------------------------------------------------------------------------

interface JsonError {
    line: number;
    col: number;
    message: string;
}

interface JsonResult {
    path: string;
    success: boolean;
    errors: JsonError[];
}

interface JsonOutput {
    results: JsonResult[];
}

const MSS_LANG = 'mimispec';
const MSS_SRC = 'mimispec';
const MSS_CFG = 'mimispec';

let mssDiags: vscode.DiagnosticCollection;
let mssChan: vscode.OutputChannel;

function initMimispec(context: vscode.ExtensionContext): void {
    mssDiags = vscode.languages.createDiagnosticCollection(MSS_SRC);
    mssChan = vscode.window.createOutputChannel('MimiSpec');
    context.subscriptions.push(mssDiags, mssChan);

    const cmd = vscode.commands.registerCommand('mimispec.validateFile', () => {
        const ed = vscode.window.activeTextEditor;
        if (ed && ed.document.languageId === MSS_LANG) {
            validateMss(ed.document);
        } else {
            vscode.window.showInformationMessage('No active MimiSpec file to validate.');
        }
    });
    context.subscriptions.push(cmd);

    context.subscriptions.push(
        vscode.workspace.onDidSaveTextDocument((doc) => {
            if (doc.languageId === MSS_LANG && getCfg<boolean>(MSS_CFG, 'validateOnSave', true)) {
                validateMss(doc);
            }
        })
    );
    context.subscriptions.push(
        vscode.workspace.onDidOpenTextDocument((doc) => {
            if (doc.languageId === MSS_LANG && getCfg<boolean>(MSS_CFG, 'validateOnOpen', true)) {
                validateMss(doc);
            }
        })
    );
    context.subscriptions.push(
        vscode.workspace.onDidCloseTextDocument((doc) => {
            if (doc.languageId === MSS_LANG) mssDiags.delete(doc.uri);
        })
    );

    // validate already-open files
    vscode.workspace.textDocuments.forEach((doc) => {
        if (doc.languageId === MSS_LANG && getCfg<boolean>(MSS_CFG, 'validateOnOpen', true)) {
            validateMss(doc);
        }
    });
}

async function validateMss(doc: vscode.TextDocument): Promise<void> {
    const bin = await findBinary('mimispec', ['target/release/mimispec', 'target/debug/mimispec']);
    if (!bin) {
        mssChan.appendLine('MimiSpec CLI binary not found. Set "mimispec.binaryPath" or build the Rust parser.');
        return;
    }
    mssChan.appendLine(`Validating ${doc.uri.fsPath} with ${bin}`);

    let output: string;
    try {
        output = await runBinary(bin, ['-j', '-'], doc.getText());
    } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        mssChan.appendLine(`Failed to run mimispec: ${msg}`);
        vscode.window.showErrorMessage(`MimiSpec validation failed: ${msg}`);
        return;
    }

    let parsed: JsonOutput;
    try {
        parsed = JSON.parse(output) as JsonOutput;
    } catch {
        mssChan.appendLine(`Unexpected parser output: ${output}`);
        return;
    }

    const result = parsed.results[0];
    if (!result) { mssDiags.delete(doc.uri); return; }

    const diags: vscode.Diagnostic[] = result.errors.map((err) => {
        const line = Math.max(1, err.line) - 1;
        const col = Math.max(1, err.col) - 1;
        const r = new vscode.Range(line, col, line, col + 1);
        const d = new vscode.Diagnostic(r, err.message, vscode.DiagnosticSeverity.Error);
        d.source = MSS_SRC;
        return d;
    });
    mssDiags.set(doc.uri, diags);

    const summary = result.success
        ? `No errors in ${path.basename(doc.uri.fsPath)}`
        : `${diags.length} error(s) in ${path.basename(doc.uri.fsPath)}`;
    mssChan.appendLine(summary);
}

// ---------------------------------------------------------------------------
// Mimi (LSP + CLI fallback)
// ---------------------------------------------------------------------------

const MIMI_LANG = 'mimi';
const MIMI_SRC = 'mimi';
const MIMI_CFG = 'mimi';

let mimiDiags: vscode.DiagnosticCollection;
let mimiChan: vscode.OutputChannel;
let lspClient: LanguageClient | undefined;

function initMimi(context: vscode.ExtensionContext): void {
    mimiDiags = vscode.languages.createDiagnosticCollection(MIMI_SRC);
    mimiChan = vscode.window.createOutputChannel('Mimi');
    context.subscriptions.push(mimiDiags, mimiChan);

    context.subscriptions.push(
        vscode.commands.registerCommand('mimi.validateFile', () => {
            const ed = vscode.window.activeTextEditor;
            if (ed && ed.document.languageId === MIMI_LANG) {
                validateMimiCli(ed.document);
            } else {
                vscode.window.showInformationMessage('No active Mimi file to check.');
            }
        })
    );
    context.subscriptions.push(
        vscode.commands.registerCommand('mimi.restartLsp', async () => {
            await stopLsp();
            await startLsp(context);
        })
    );

    // Start LSP if enabled
    if (getCfg<boolean>(MIMI_CFG, 'enableLsp', true)) {
        startLsp(context);
    }

    // Fallback CLI validation on save/open when LSP is not active
    context.subscriptions.push(
        vscode.workspace.onDidSaveTextDocument((doc) => {
            if (doc.languageId !== MIMI_LANG) return;
            if (!lspActive() && getCfg<boolean>(MIMI_CFG, 'validateOnSave', true)) {
                validateMimiCli(doc);
            }
        })
    );
    context.subscriptions.push(
        vscode.workspace.onDidOpenTextDocument((doc) => {
            if (doc.languageId !== MIMI_LANG) return;
            if (!lspActive() && getCfg<boolean>(MIMI_CFG, 'validateOnOpen', true)) {
                validateMimiCli(doc);
            }
        })
    );
    context.subscriptions.push(
        vscode.workspace.onDidCloseTextDocument((doc) => {
            if (doc.languageId === MIMI_LANG) mimiDiags.delete(doc.uri);
        })
    );

    // Validate already-open files (only if LSP is off)
    if (!lspActive()) {
        vscode.workspace.textDocuments.forEach((doc) => {
            if (doc.languageId === MIMI_LANG && getCfg<boolean>(MIMI_CFG, 'validateOnOpen', true)) {
                validateMimiCli(doc);
            }
        });
    }
}

function lspActive(): boolean {
    return lspClient !== undefined && lspClient.state === LspState.Running;
}

async function startLsp(context: vscode.ExtensionContext): Promise<void> {
    const bin = await findBinary('mimi', ['target/release/mimi', 'target/debug/mimi']);
    if (!bin) {
        mimiChan.appendLine('Mimi binary not found. Set "mimi.binaryPath" or build the Mimi compiler.');
        return;
    }

    const serverOptions: ServerOptions = {
        command: bin,
        args: ['lsp'],
        options: { env: { ...process.env } },
    };

    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: 'file', language: MIMI_LANG }],
        diagnosticCollectionName: MIMI_SRC,
        outputChannel: mimiChan,
        traceOutputChannel: mimiChan,
    };

    lspClient = new LanguageClient('mimi-lsp', 'Mimi Language Server', serverOptions, clientOptions);

    lspClient.onDidChangeState((ev) => {
        mimiChan.appendLine(`Mimi LSP state: ${LspState[ev.oldState]} → ${LspState[ev.newState]}`);
    });

    try {
        await lspClient.start();
        mimiChan.appendLine('Mimi LSP started.');
    } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        mimiChan.appendLine(`Failed to start Mimi LSP: ${msg}`);
        vscode.window.showWarningMessage(`Mimi LSP failed to start: ${msg}. Falling back to CLI validation.`);
    }
}

async function stopLsp(): Promise<void> {
    if (lspClient) {
        try {
            await lspClient.stop();
        } catch { /* ignore */ }
        lspClient = undefined;
    }
}

async function validateMimiCli(doc: vscode.TextDocument): Promise<void> {
    const bin = await findBinary('mimi', ['target/release/mimi', 'target/debug/mimi']);
    if (!bin) {
        mimiChan.appendLine('Mimi binary not found. Set "mimi.binaryPath" or build the Mimi compiler.');
        return;
    }
    mimiChan.appendLine(`Checking ${doc.uri.fsPath} with ${bin}`);

    let stderr: string;
    try {
        stderr = await runBinaryStderr(bin, ['check', doc.uri.fsPath]);
    } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        mimiChan.appendLine(`Failed to run mimi: ${msg}`);
        return;
    }

    // Parse Rustc-style diagnostics: error[EXXXX] message\n --> file:line:col
    const errorRegex = /error\[E\d+\]\s+(.+)\n\s+-->\s+(?:\S+):(\d+):(\d+)/g;
    const diags: vscode.Diagnostic[] = [];
    let match: RegExpExecArray | null;
    while ((match = errorRegex.exec(stderr)) !== null) {
        const msg = match[1].trim();
        const line = Math.max(1, parseInt(match[2], 10)) - 1;
        const col = Math.max(1, parseInt(match[3], 10)) - 1;
        const r = new vscode.Range(line, col, line, col + 1);
        const d = new vscode.Diagnostic(r, msg, vscode.DiagnosticSeverity.Error);
        d.source = MIMI_SRC;
        diags.push(d);
    }

    mimiDiags.set(doc.uri, diags);

    if (diags.length === 0) {
        mimiChan.appendLine(`No errors in ${path.basename(doc.uri.fsPath)}`);
    } else {
        mimiChan.appendLine(`${diags.length} error(s) in ${path.basename(doc.uri.fsPath)}`);
    }
}

// ---------------------------------------------------------------------------
// Shared utilities
// ---------------------------------------------------------------------------

function getCfg<T>(section: string, key: string, defaultValue: T): T {
    return vscode.workspace.getConfiguration(section).get<T>(key, defaultValue);
}

async function findBinary(section: string, candidates: string[]): Promise<string | undefined> {
    const configured = getCfg<string | null>(section, 'binaryPath', null);
    if (configured) {
        if (fs.existsSync(configured)) return configured;
    }

    const folders = vscode.workspace.workspaceFolders;
    if (!folders) return undefined;

    for (const folder of folders) {
        for (const c of candidates) {
            const full = path.join(folder.uri.fsPath, c);
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
        child.stdout.on('data', (d: Buffer) => { stdout += d.toString('utf-8'); });
        child.stderr.on('data', (d: Buffer) => { stderr += d.toString('utf-8'); });
        child.on('error', (err) => reject(err));
        child.on('close', (code) => {
            if (code !== 0 && code !== 1) reject(new Error(stderr || `${bin} exited with code ${code}`));
            else resolve(stdout);
        });
        child.stdin.end(input, 'utf-8');
    });
}

function runBinaryStderr(bin: string, args: string[]): Promise<string> {
    return new Promise((resolve, reject) => {
        const child = spawn(bin, args, { stdio: ['pipe', 'pipe', 'pipe'] });
        let stderr = '';
        child.stderr.on('data', (d: Buffer) => { stderr += d.toString('utf-8'); });
        child.on('error', (err) => reject(err));
        child.on('close', (code) => {
            if (code !== 0 && code !== 1) reject(new Error(stderr || `${bin} exited with code ${code}`));
            else resolve(stderr);
        });
    });
}

// ---------------------------------------------------------------------------
// Activation / Deactivation
// ---------------------------------------------------------------------------

export function activate(context: vscode.ExtensionContext): void {
    initMimispec(context);
    initMimi(context);
}

export async function deactivate(): Promise<void> {
    await stopLsp();
    if (mssDiags) mssDiags.dispose();
    if (mimiDiags) mimiDiags.dispose();
    if (mssChan) mssChan.dispose();
    if (mimiChan) mimiChan.dispose();
}

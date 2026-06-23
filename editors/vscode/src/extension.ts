import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import { spawn } from 'child_process';

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

const LANG = 'mimispec';
const CFG = 'mimispec';

let diags: vscode.DiagnosticCollection;
let chan: vscode.OutputChannel;

function initValidation(context: vscode.ExtensionContext): void {
    diags = vscode.languages.createDiagnosticCollection(LANG);
    chan = vscode.window.createOutputChannel('MimiSpec');
    context.subscriptions.push(diags, chan);

    const cmd = vscode.commands.registerCommand('mimispec.validateFile', () => {
        const ed = vscode.window.activeTextEditor;
        if (ed && ed.document.languageId === LANG) {
            validate(ed.document);
        } else {
            vscode.window.showInformationMessage('No active MimiSpec file to validate.');
        }
    });
    context.subscriptions.push(cmd);

    context.subscriptions.push(
        vscode.workspace.onDidSaveTextDocument((doc) => {
            if (doc.languageId === LANG && getCfg<boolean>(CFG, 'validateOnSave', true)) {
                validate(doc);
            }
        })
    );
    context.subscriptions.push(
        vscode.workspace.onDidOpenTextDocument((doc) => {
            if (doc.languageId === LANG && getCfg<boolean>(CFG, 'validateOnOpen', true)) {
                validate(doc);
            }
        })
    );
    context.subscriptions.push(
        vscode.workspace.onDidCloseTextDocument((doc) => {
            if (doc.languageId === LANG) diags.delete(doc.uri);
        })
    );

    vscode.workspace.textDocuments.forEach((doc) => {
        if (doc.languageId === LANG && getCfg<boolean>(CFG, 'validateOnOpen', true)) {
            validate(doc);
        }
    });
}

async function validate(doc: vscode.TextDocument): Promise<void> {
    const bin = await findBinary(['target/release/mimispec', 'target/debug/mimispec']);
    if (!bin) {
        chan.appendLine('MimiSpec CLI binary not found. Set "mimispec.binaryPath" or build the Rust parser.');
        return;
    }
    chan.appendLine(`Validating ${doc.uri.fsPath} with ${bin}`);

    let output: string;
    try {
        output = await runBinary(bin, ['--json', '-'], doc.getText());
    } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        chan.appendLine(`Failed to run mimispec: ${msg}`);
        vscode.window.showErrorMessage(`MimiSpec validation failed: ${msg}`);
        return;
    }

    let parsed: JsonOutput;
    try {
        parsed = JSON.parse(output) as JsonOutput;
    } catch {
        chan.appendLine(`Unexpected parser output: ${output}`);
        return;
    }

    const result = parsed.results[0];
    if (!result) { diags.delete(doc.uri); return; }

    const ds: vscode.Diagnostic[] = result.errors.map((err) => {
        const line = Math.max(1, err.line) - 1;
        const col = Math.max(1, err.col) - 1;
        const r = new vscode.Range(line, col, line, col + 1);
        const d = new vscode.Diagnostic(r, err.message, vscode.DiagnosticSeverity.Error);
        d.source = LANG;
        return d;
    });
    diags.set(doc.uri, ds);

    const summary = result.success
        ? `No errors in ${path.basename(doc.uri.fsPath)}`
        : `${ds.length} error(s) in ${path.basename(doc.uri.fsPath)}`;
    chan.appendLine(summary);
}

function getCfg<T>(section: string, key: string, defaultValue: T): T {
    return vscode.workspace.getConfiguration(section).get<T>(key, defaultValue);
}

async function findBinary(candidates: string[]): Promise<string | undefined> {
    const configured = getCfg<string | null>(CFG, 'binaryPath', null);
    if (configured && fs.existsSync(configured)) return configured;

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

export function activate(context: vscode.ExtensionContext): void {
    initValidation(context);
}

export function deactivate(): void {
    if (diags) diags.dispose();
    if (chan) chan.dispose();
}

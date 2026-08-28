import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient";

let client: LanguageClient | undefined;

/**
 * Locates the `sandbox` CLI binary: `SANDBOX_BIN` env var first,
 * then `sandbox` on PATH, then the project's target dir (debug/release).
 */
async function findSandboxBinary(): Promise<string> {
  const fromEnv = process.env.SANDBOX_BIN;
  if (fromEnv) return fromEnv;

  const workspaceRoot =
    vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? process.cwd();
  const candidates = [
    `${workspaceRoot}/target/debug/sandbox`,
    `${workspaceRoot}/target/release/sandbox`,
  ];
  for (const c of candidates) {
    try {
      await vscode.workspace.fs.stat(vscode.Uri.file(c));
      return c;
    } catch {
      /* not found */
    }
  }
  return "sandbox"; // let PATH resolve it
}

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  const bin = await findSandboxBinary();

  const serverOptions: ServerOptions = {
    command: bin,
    args: ["lsp"],
    transport: TransportKind.stdio,
    options: { detached: false },
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ language: "sandbox" }],
    synchronize: {
      configurationSection: "sandbox",
    },
    outputChannelName: "Sandbox Language Server",
  };

  client = new LanguageClient("sandbox-lsp", "Sandbox Language Server", serverOptions, clientOptions);
  await client.start();

  context.subscriptions.push(
    vscode.commands.registerCommand("sandbox.restartLsp", async () => {
      await client?.stop();
      client = new LanguageClient(
        "sandbox-lsp",
        "Sandbox Language Server",
        serverOptions,
        clientOptions
      );
      await client.start();
      vscode.window.showInformationMessage("Sandbox language server restarted");
    })
  );

  vscode.window.showInformationMessage("Sandbox extension activated (LSP: " + bin + ")");
}

export function deactivate(): Thenable<void> | undefined {
  return client?.stop();
}

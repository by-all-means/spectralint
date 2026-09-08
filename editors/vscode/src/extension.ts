import {
  ExtensionContext,
  workspace,
} from "vscode";

import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

export function activate(context: ExtensionContext) {
  const config = workspace.getConfiguration("spectralint");
  const command = config.get<string>("path", "spectralint");

  const serverOptions: ServerOptions = {
    command,
    args: ["lsp"],
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: "file", language: "markdown" },
      { scheme: "file", pattern: "**/*.mdc" },
      { scheme: "file", pattern: "**/.cursorrules" },
      { scheme: "file", pattern: "**/.clinerules" },
      { scheme: "file", pattern: "**/.windsurfrules" },
    ],
  };

  client = new LanguageClient(
    "spectralint",
    "Spectralint",
    serverOptions,
    clientOptions
  );

  client.start();
}

export function deactivate(): Thenable<void> | undefined {
  return client?.stop();
}

const vscode = require("vscode");
const { spawn } = require("child_process");

// Each kind of Quill word has its own color. Dark colors sit on a dark editor.
// Light colors sit on paper. Change PALETTE_MARK when these colors change,
// or an editor that already saved the old set will keep it.
//
// A word gets a color only once the language has it.
const DARK = {
  say: "#F0A35E",
  when: "#7EAEF5",
  truth: "#5ECA8C",
  bind: "#C9A6F2",
  check: "#F0A0B4",
  logic: "#5EC4C0",
  loop: "#C6D86A",
  list: "#D4896A",
  record: "#A8C5A0",
  of: "#E8C547",
  at: "#B07CFF",
  ask: "#6EC1D6",
  fn: "#E8A0D4",
  maybe: "#F0B090",
  failure: "#E08A8A",
  contract: "#C8A06A",
  rule: "#6AA8C8",
  money: "#8FD3B5",
  module: "#A8C0D8",
  permit: "#C47848",
  ask: "#E57BE0",
  number: "#E7C98A",
  string: "#F3E6D0",
  name: "#F7F1E8",
  comment: "#9A9186",
  operator: "#B7A898",
  escape: "#FFBE7A",
};

const LIGHT = {
  say: "#C45C12",
  when: "#1D408A",
  truth: "#157A42",
  bind: "#6E4A96",
  check: "#9A3D6A",
  logic: "#0F6E68",
  loop: "#5A6A14",
  list: "#8C4E32",
  record: "#3E5C32",
  of: "#8A5A00",
  at: "#5C2E9A",
  ask: "#15556A",
  fn: "#8A3D78",
  maybe: "#A85A38",
  failure: "#A33B3B",
  contract: "#7A5A20",
  rule: "#1A5270",
  money: "#2E7A5C",
  module: "#3D5270",
  permit: "#8C4E20",
  ask: "#8E2C8A",
  number: "#9A4A12",
  string: "#6B3A22",
  name: "#1C1916",
  comment: "#8A8175",
  operator: "#5C4632",
  escape: "#C45C12",
};

const PALETTE_MARK = { dark: "#E57BE0", light: "#8E2C8A" };

const SCOPE = {
  say: "keyword.other.say.quill",
  when: "keyword.control.conditional.quill",
  truth: "constant.language.quill",
  bind: "keyword.other.bind.quill",
  check: "keyword.other.check.quill, keyword.control.example.quill",
  logic: "keyword.operator.logical.quill",
  loop: "keyword.control.loop.quill",
  list: "keyword.other.list.quill",
  record: "keyword.other.record.quill",
  of: "keyword.other.of.quill",
  at: "keyword.other.at.quill",
  ask: "keyword.other.ask.quill",
  fn: "keyword.other.function.quill",
  maybe: "keyword.other.maybe.quill",
  failure: "keyword.other.failure.quill",
  contract: "keyword.other.contract.quill, variable.language.result.quill",
  rule: "keyword.other.rule.quill",
  money: "keyword.other.money.quill",
  module: "keyword.other.module.quill",
  permit: "keyword.other.permit.quill",
  ask: "keyword.other.ask-model.quill",
  number: "constant.numeric.quill",
  string:
    "string.quoted.double.quill, string.quoted.triple.quill, punctuation.definition.string.begin.quill, punctuation.definition.string.end.quill",
  name: "variable.other.quill",
  comment: "comment.line.double-dash.quill",
  operator:
    "keyword.operator.quill, meta.interpolation.quill, punctuation.definition.interpolation.begin.quill, punctuation.definition.interpolation.end.quill",
  escape: "constant.character.escape.quill",
};

function isLight(kind) {
  return kind === vscode.ColorThemeKind.Light || kind === vscode.ColorThemeKind.HighContrastLight;
}

function quillRules(palette) {
  return Object.keys(SCOPE).map((name) => ({
    scope: SCOPE[name],
    settings: { foreground: palette[name] },
  }));
}

/** Keep every other language's colors. Replace only rules that belong to Quill. */
async function applyColors(kind) {
  const palette = isLight(kind) ? LIGHT : DARK;
  const marker = isLight(kind) ? PALETTE_MARK.light : PALETTE_MARK.dark;
  const config = vscode.workspace.getConfiguration();
  const current = config.get("editor.tokenColorCustomizations") || {};
  const rules = Array.isArray(current.textMateRules) ? current.textMateRules : [];
  const already = rules.some(
    (rule) => rule.scope === "meta.quill.palette" && rule.settings && rule.settings.foreground === marker,
  );
  if (already) return;
  const kept = rules.filter((rule) => !String(rule.scope || "").includes("quill"));
  const next = kept.concat(quillRules(palette), [
    { scope: "meta.quill.palette", settings: { foreground: marker } },
  ]);
  try {
    await config.update(
      "editor.tokenColorCustomizations",
      { ...current, textMateRules: next },
      vscode.ConfigurationTarget.Global,
    );
  } catch (_err) {
    // The editor refused the write. Quill still colors by scope, using the theme.
  }
}

function frame(message) {
  const body = JSON.stringify(message);
  return `Content-Length: ${Buffer.byteLength(body)}\r\n\r\n${body}`;
}

function startChecker(context) {
  const quill = process.env.QUILL_PATH || "quill";
  let child;
  try {
    child = spawn(quill, ["lsp"], { stdio: ["pipe", "pipe", "ignore"] });
  } catch (_err) {
    return;
  }
  child.on("error", () => {});

  const problems = vscode.languages.createDiagnosticCollection("quill");
  context.subscriptions.push(problems);
  let buffer = Buffer.alloc(0);
  let nextId = 1;
  const open = new Map();

  function send(message) {
    if (!child.stdin.writable) return;
    child.stdin.write(frame(message));
  }

  function sendDocument(uri, text) {
    send({
      jsonrpc: "2.0",
      method: "textDocument/didChange",
      params: {
        textDocument: { uri },
        contentChanges: [{ text }],
      },
    });
  }

  child.stdout.on("data", (chunk) => {
    buffer = Buffer.concat([buffer, chunk]);
    while (true) {
      const headerEnd = buffer.indexOf("\r\n\r\n");
      if (headerEnd < 0) return;
      const header = buffer.slice(0, headerEnd).toString("utf8");
      const match = header.match(/Content-Length:\s*(\d+)/i);
      if (!match) {
        buffer = buffer.slice(headerEnd + 4);
        continue;
      }
      const length = Number(match[1]);
      const start = headerEnd + 4;
      if (buffer.length < start + length) return;
      const body = buffer.slice(start, start + length).toString("utf8");
      buffer = buffer.slice(start + length);
      let message;
      try {
        message = JSON.parse(body);
      } catch (_err) {
        continue;
      }
      if (message.method === "textDocument/publishDiagnostics" && message.params) {
        const uri = vscode.Uri.parse(message.params.uri);
        const diagnostics = (message.params.diagnostics || []).map(
          (item) =>
            new vscode.Diagnostic(
              new vscode.Range(
                item.range.start.line,
                item.range.start.character,
                item.range.end.line,
                item.range.end.character,
              ),
              item.message,
              vscode.DiagnosticSeverity.Error,
            ),
        );
        diagnostics.forEach((item) => {
          item.source = "quill";
        });
        problems.set(uri, diagnostics);
      }
    }
  });

  send({
    jsonrpc: "2.0",
    id: nextId++,
    method: "initialize",
    params: { processId: process.pid, capabilities: {}, rootUri: null },
  });
  send({ jsonrpc: "2.0", method: "initialized", params: {} });

  function watch(document) {
    if (document.languageId !== "quill") return;
    const uri = document.uri.toString();
    if (!open.has(uri)) {
      open.set(uri, true);
      send({
        jsonrpc: "2.0",
        method: "textDocument/didOpen",
        params: {
          textDocument: { uri, languageId: "quill", version: document.version, text: document.getText() },
        },
      });
      return;
    }
    sendDocument(uri, document.getText());
  }

  vscode.workspace.textDocuments.forEach(watch);
  context.subscriptions.push(vscode.workspace.onDidOpenTextDocument(watch));
  context.subscriptions.push(
    vscode.workspace.onDidChangeTextDocument((event) => watch(event.document)),
  );
  context.subscriptions.push(
    vscode.workspace.onDidCloseTextDocument((document) => {
      const uri = document.uri.toString();
      open.delete(uri);
      problems.delete(document.uri);
      send({
        jsonrpc: "2.0",
        method: "textDocument/didClose",
        params: { textDocument: { uri } },
      });
    }),
  );
  context.subscriptions.push({
    dispose() {
      send({ jsonrpc: "2.0", method: "exit" });
      child.kill();
    },
  });
}

function activate(context) {
  applyColors(vscode.window.activeColorTheme.kind);
  context.subscriptions.push(
    vscode.window.onDidChangeActiveColorTheme((theme) => applyColors(theme.kind)),
  );
  startChecker(context);
}

function deactivate() {}

module.exports = { activate, deactivate };

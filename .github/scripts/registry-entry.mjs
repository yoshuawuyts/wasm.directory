// Parse a "Registry entry" issue form and add the requested component or
// interface to the matching `registry/<namespace>.toml` file.
//
// Inputs (environment variables):
//   ISSUE_BODY     - the raw markdown body of the issue form submission
//
// Outputs (written to $GITHUB_OUTPUT):
//   ok             - "true" if the registry file was created/updated
//   message        - human-readable status/error message for an issue comment
//   new_namespace  - "true" when a brand new namespace file was created
//   namespace      - the parsed namespace
//   kind           - "component" or "interface"
//   package        - the parsed package name
//   repository     - the parsed repository path
//
// The script always exits 0; callers branch on the `ok` output. This keeps the
// workflow in control of commenting and labelling.

import { readFileSync, writeFileSync, existsSync, appendFileSync } from "node:fs";
import { join } from "node:path";

const NAME_RE = /^[A-Za-z0-9][A-Za-z0-9._-]*$/;
const REPO_RE = /^[A-Za-z0-9][A-Za-z0-9._/-]*$/;
const REGISTRY_RE = /^[A-Za-z0-9][A-Za-z0-9._:/-]*$/;

const outputs = {};

function setOutput(key, value) {
  outputs[key] = value;
}

function flushOutputs() {
  const file = process.env.GITHUB_OUTPUT;
  if (!file) {
    for (const [k, v] of Object.entries(outputs)) console.log(`${k}=${v}`);
    return;
  }
  let data = "";
  for (const [k, v] of Object.entries(outputs)) {
    const str = String(v);
    if (str.includes("\n")) {
      const delim = `EOF_${k}_${Math.random().toString(36).slice(2)}`;
      data += `${k}<<${delim}\n${str}\n${delim}\n`;
    } else {
      data += `${k}=${str}\n`;
    }
  }
  appendFileSync(file, data);
}

function fail(message) {
  setOutput("ok", "false");
  setOutput("message", message);
  flushOutputs();
  process.exit(0);
}

// Parse the GitHub issue-form body into a { label: value } map. Form responses
// are rendered as `### <Label>` headings followed by the value on later lines.
function parseForm(body) {
  const fields = {};
  let label = null;
  let buffer = [];
  const commit = () => {
    if (label !== null) {
      fields[label] = buffer.join("\n").trim();
    }
  };
  for (const rawLine of body.split(/\r?\n/)) {
    const heading = rawLine.match(/^###\s+(.*\S)\s*$/);
    if (heading) {
      commit();
      label = heading[1].trim().toLowerCase();
      buffer = [];
    } else if (label !== null) {
      buffer.push(rawLine);
    }
  }
  commit();
  return fields;
}

function normalize(value) {
  if (value === undefined) return "";
  const trimmed = value.trim();
  if (trimmed === "_No response_") return "";
  return trimmed;
}

// Walk a TOML file's array-of-tables and collect { kind, name } pairs plus the
// declared namespace name. Intentionally lightweight: only understands the flat
// shape used by registry files.
function inspectToml(text) {
  let namespaceName = null;
  let section = null;
  const entries = [];
  let current = null;
  for (const rawLine of text.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (line === "[namespace]") {
      section = "namespace";
      current = null;
      continue;
    }
    const array = line.match(/^\[\[(component|interface)\]\]$/);
    if (array) {
      section = array[1];
      current = { kind: array[1], name: null };
      entries.push(current);
      continue;
    }
    if (line.startsWith("[")) {
      section = null;
      current = null;
      continue;
    }
    const kv = line.match(/^name\s*=\s*"([^"]*)"/);
    if (kv) {
      if (section === "namespace") namespaceName = kv[1];
      else if (current) current.name = kv[1];
    }
  }
  return { namespaceName, entries };
}

function buildEntry(kind, name, repository) {
  return `[[${kind}]]\nname = "${name}"\nrepository = "${repository}"\n`;
}

function main() {
  const body = process.env.ISSUE_BODY || "";
  const fields = parseForm(body);

  const kind = normalize(fields["kind"]).toLowerCase();
  const namespace = normalize(fields["namespace"]);
  const pkg = normalize(fields["package name"]);
  const repository = normalize(fields["repository"]);
  const registry = normalize(fields["oci registry (new namespaces only)"]);

  if (kind !== "component" && kind !== "interface") {
    fail("**Kind** must be either `component` or `interface`.");
  }
  if (!NAME_RE.test(namespace)) {
    fail(
      "**Namespace** is missing or invalid. Use letters, numbers, `.`, `_`, or `-` (e.g. `wasi`).",
    );
  }
  if (!NAME_RE.test(pkg)) {
    fail(
      "**Package name** is missing or invalid. Use letters, numbers, `.`, `_`, or `-` (e.g. `http`).",
    );
  }
  if (!REPO_RE.test(repository)) {
    fail(
      "**Repository** is missing or invalid. Use a path like `components/my-package`.",
    );
  }

  setOutput("kind", kind);
  setOutput("namespace", namespace);
  setOutput("package", pkg);
  setOutput("repository", repository);

  const file = join("registry", `${namespace}.toml`);
  const exists = existsSync(file);

  if (!exists) {
    if (!REGISTRY_RE.test(registry)) {
      fail(
        `Namespace \`${namespace}\` does not exist yet, so the **OCI registry** ` +
          "field is required (e.g. `ghcr.io/my-org`). Please edit the issue and " +
          "fill it in.",
      );
    }
    const content =
      `[namespace]\nname = "${namespace}"\nregistry = "${registry}"\n\n` +
      buildEntry(kind, pkg, repository);
    writeFileSync(file, content);
    setOutput("new_namespace", "true");
    setOutput("ok", "true");
    setOutput(
      "message",
      `Created new namespace \`${namespace}\` with ${kind} \`${pkg}\`. ` +
        "Because this creates a **new namespace**, the pull request needs " +
        "manual review before it can be merged.",
    );
    flushOutputs();
    return;
  }

  const existing = readFileSync(file, "utf8");
  const { namespaceName, entries } = inspectToml(existing);

  if (namespaceName && namespaceName !== namespace) {
    fail(
      `\`${file}\` declares namespace \`${namespaceName}\`, which does not ` +
        `match the requested namespace \`${namespace}\`.`,
    );
  }

  const duplicate = entries.some((e) => e.kind === kind && e.name === pkg);
  if (duplicate) {
    fail(
      `\`${namespace}\` already has a ${kind} named \`${pkg}\`. Nothing to do.`,
    );
  }

  const prefix = existing.endsWith("\n") ? "" : "\n";
  writeFileSync(file, `${existing}${prefix}\n${buildEntry(kind, pkg, repository)}`);
  setOutput("new_namespace", "false");
  setOutput("ok", "true");
  setOutput(
    "message",
    `Added ${kind} \`${pkg}\` to existing namespace \`${namespace}\`. ` +
      "Because the namespace already exists, the pull request can be merged " +
      "automatically.",
  );
  flushOutputs();
}

main();

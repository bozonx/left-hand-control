import { readFileSync, existsSync } from 'fs';

const TS_PATH = 'utils/systemMacros.ts';
const RS_PATH = 'src-tauri/src/mapper/system_macros.rs';

function exit(msg) {
  console.error(msg);
  process.exit(1);
}

function extractQuotedStrings(src) {
  const out = [];
  const re = /"([^"]*)"/g;
  let m;
  while ((m = re.exec(src)) !== null) out.push(m[1]);
  return out;
}

function parseTsMacros(path) {
  if (!existsSync(path)) exit(`Missing ${path}`);
  const text = readFileSync(path, 'utf-8');
  const macros = new Map();
  // Each macro block starts with id: "..." and has steps: s(...)
  const blockRe = /id:\s*"([^"]+)"[\s\S]*?steps:\s*s\(([^)]*)\)/g;
  let m;
  while ((m = blockRe.exec(text)) !== null) {
    const id = m[1];
    const steps = extractQuotedStrings(m[2]);
    macros.set(id, steps);
  }
  return macros;
}

function parseRsMacros(path) {
  if (!existsSync(path)) exit(`Missing ${path}`);
  const text = readFileSync(path, 'utf-8');
  const macros = new Map();
  const blockRe = /id:\s*"([^"]+)"[\s\S]*?steps:\s*&\[([^\]]*)\]/g;
  let m;
  while ((m = blockRe.exec(text)) !== null) {
    const id = m[1];
    const steps = extractQuotedStrings(m[2]);
    macros.set(id, steps);
  }
  return macros;
}

const tsMacros = parseTsMacros(TS_PATH);
const rsMacros = parseRsMacros(RS_PATH);

const tsIds = new Set(tsMacros.keys());
const rsIds = new Set(rsMacros.keys());

let errors = 0;

for (const id of tsIds) {
  if (!rsIds.has(id)) {
    console.error(`Only in TS: ${id}`);
    errors++;
  }
}
for (const id of rsIds) {
  if (!tsIds.has(id)) {
    console.error(`Only in Rust: ${id}`);
    errors++;
  }
}

for (const id of tsIds) {
  if (!rsIds.has(id)) continue;
  const tsSteps = tsMacros.get(id);
  const rsSteps = rsMacros.get(id);
  if (JSON.stringify(tsSteps) !== JSON.stringify(rsSteps)) {
    console.error(
      `Mismatched steps for ${id}:\n  TS: ${JSON.stringify(tsSteps)}\n  RS: ${JSON.stringify(rsSteps)}`
    );
    errors++;
  }
}

if (errors) {
  exit(`\n${errors} system-macro sync error(s). Update both ${TS_PATH} and ${RS_PATH}.`);
}

console.log(`System macros in sync: ${tsIds.size} ids, ${tsIds.size === rsIds.size ? 'OK' : 'MISMATCH'}`);

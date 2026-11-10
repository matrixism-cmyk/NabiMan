// Finds Korean words written straight into components instead of going
// through useT(): Hangul in string, template or JSX text outside comments.
// Translation tables, tests and vendor code are not checked. Text that is
// data rather than UI (sample content, patterns matching speech or server
// output, names shown in their own script) can stay when `// i18n-exempt:
// <reason>` is on its line, or on the line above it or above any construct
// holding it up to its statement (so one comment covers a whole table);
// a whole file can opt out with `// i18n-exempt-file: <reason>` near its top.
const fs = require('fs');
const path = require('path');
const ts = require('typescript');

const src = path.join(__dirname, '..', 'src');
const SKIP_DIRS = ['vendor', path.join('i18n', 'translations')].map((d) => path.join(src, d));
const SKIP_FILES = [path.join(src, 'shared', 'i18n', 'strings.ts')];
const HANGUL = /[ᄀ-ᇿ㄰-㆏가-힯]/;
const EXEMPT = /i18n-exempt:\s*\S/;
const EXEMPT_FILE = /i18n-exempt-file:\s*\S/;

function walk(dir, out = []) {
  if (SKIP_DIRS.includes(dir)) return out;
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(full, out);
    else if (/\.tsx?$/.test(entry.name) && !/\.(test|spec)\.tsx?$/.test(entry.name)
      && !/\.d\.ts$/.test(entry.name) && entry.name !== 'setupTests.ts'
      && !SKIP_FILES.includes(full)) out.push(full);
  }
  return out;
}

const TEXT_KINDS = new Set([
  ts.SyntaxKind.StringLiteral,
  ts.SyntaxKind.NoSubstitutionTemplateLiteral,
  ts.SyntaxKind.TemplateHead,
  ts.SyntaxKind.TemplateMiddle,
  ts.SyntaxKind.TemplateTail,
  ts.SyntaxKind.JsxText,
]);

/** Lines (1-based) of one file holding Hangul in text, minus the exempt ones. */
function scan(file) {
  const text = fs.readFileSync(file, 'utf8');
  if (!HANGUL.test(text)) return { hits: [], exempt: 0 };
  if (EXEMPT_FILE.test(text.split('\n').slice(0, 15).join('\n'))) return { hits: [], exempt: 0, fileExempt: true };
  const lines = text.split('\n');
  const kind = file.endsWith('.tsx') ? ts.ScriptKind.TSX : ts.ScriptKind.TS;
  const sf = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true, kind);
  const lineOf = (pos) => sf.getLineAndCharacterOfPosition(pos).line;
  const marked = (l) => EXEMPT.test(lines[l]) || (l > 0 && EXEMPT.test(lines[l - 1]));
  /** Marked on the text's own line, or above it or any construct around it up to its statement. */
  const exempted = (node, line) => {
    if (marked(line)) return true;
    for (let at = node; at && at.kind !== ts.SyntaxKind.SourceFile; at = at.parent) {
      if (marked(lineOf(at.getStart(sf)))) return true;
      if (ts.isStatement(at) || ts.isClassElement(at)) break;
    }
    return false;
  };
  const hits = new Set();
  const exempt = new Set();
  const visit = (node) => {
    if (TEXT_KINDS.has(node.kind) && HANGUL.test(node.getText(sf))) {
      const start = lineOf(node.getStart(sf));
      const end = lineOf(node.getEnd());
      for (let l = start; l <= end; l++) {
        if (HANGUL.test(lines[l])) (exempted(node, l) ? exempt : hits).add(l);
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(sf);
  for (const l of exempt) hits.delete(l);
  return { hits: [...hits].sort((a, b) => a - b).map((l) => l + 1), exempt: exempt.size };
}

const byFile = [];
let total = 0;
let exempt = 0;
let filesExempt = 0;
for (const file of walk(src)) {
  const r = scan(file);
  exempt += r.exempt;
  if (r.fileExempt) filesExempt++;
  if (r.hits.length) {
    byFile.push([path.relative(src, file), r.hits]);
    total += r.hits.length;
  }
}

// Every file:line when there are few (as after a new slip) or with --list.
const verbose = process.argv.includes('--list') || total <= 40;
if (total) {
  byFile.sort((a, b) => b[1].length - a[1].length);
  console.error(`Hardcoded Korean outside the translations: ${total} line(s) in ${byFile.length} file(s).`);
  for (const [file, hits] of byFile) {
    console.error(`  ${file}: ${hits.length}`);
    if (verbose) for (const line of hits) console.error(`    src/${file}:${line}`);
  }
  console.error('Move the words into src/i18n/translations and use t(), or mark data with `// i18n-exempt: <reason>`.');
  process.exit(1);
}
console.log(`No hardcoded Korean outside the translations (${exempt} exempt line(s), ${filesExempt} exempt file(s)).`);

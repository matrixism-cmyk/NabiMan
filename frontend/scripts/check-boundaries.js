// Keeps the parts of the frontend apart (docs/remote-separation/shared-parts.md):
// shared/ stands on its own, and Remote takes from NabiMan only what is in
// shared/. Tests may use NabiMan's full set of words, so they are not checked.
const fs = require('fs');
const path = require('path');

const src = path.join(__dirname, '..', 'src');
const SHARED = ['shared'];
const REMOTE = ['components/remote', 'components/terminal', 'components/claude'];
const rules = [
  { name: 'shared', from: SHARED, allowed: SHARED },
  { name: 'Remote', from: REMOTE, allowed: [...REMOTE, ...SHARED, 'vendor'] },
];
const SPEC = /(?:\bfrom\s*|\bimport\s*\(\s*|\bimport\s+|\brequire\s*\(\s*)['"](\.\.?\/[^'"]*)['"]/g;

const under = (file, dirs) => dirs.some((d) => {
  const dir = path.join(src, d);
  return file === dir || file.startsWith(dir + path.sep);
});

function walk(dir, out = []) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(full, out);
    else if (/\.(tsx?|jsx?)$/.test(entry.name) && !/\.test\./.test(entry.name)) out.push(full);
  }
  return out;
}

const problems = [];
for (const rule of rules) {
  for (const dir of rule.from) {
    for (const file of walk(path.join(src, dir))) {
      const text = fs.readFileSync(file, 'utf8');
      for (const match of text.matchAll(SPEC)) {
        const target = path.resolve(path.dirname(file), match[1]);
        if (!under(target, rule.allowed)) {
          problems.push(`${path.relative(src, file)}: ${rule.name} imports ${match[1]}`);
        }
      }
    }
  }
}

if (problems.length) {
  console.error('Imports across the frontend boundaries:\n  ' + problems.join('\n  '));
  process.exit(1);
}
console.log('Frontend boundaries hold.');

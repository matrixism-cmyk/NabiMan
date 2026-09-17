// The browser has no `process`, so a leftover `process.env` reference in the
// bundle is a ReferenceError that only shows up when a user opens the page.
// The build is the only place this can be caught cheaply.
const fs = require('fs');
const path = require('path');

const jsDir = path.join(__dirname, '..', 'build', 'static', 'js');
const forbidden = [/process\.env\./, /%PUBLIC_URL%/];

const files = [];
(function walk(dir) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(full);
    else if (entry.name.endsWith('.js')) files.push(full);
  }
})(jsDir);

const problems = [];
for (const file of files) {
  const text = fs.readFileSync(file, 'utf8');
  for (const pattern of forbidden) {
    const hit = text.match(pattern);
    if (hit) problems.push(`${path.relative(process.cwd(), file)}: ${hit[0]}`);
  }
}

if (problems.length) {
  console.error('Build check failed — these never survive into a browser:');
  problems.forEach((p) => console.error(`  ${p}`));
  process.exit(1);
}
console.log(`Build check passed (${files.length} bundles).`);

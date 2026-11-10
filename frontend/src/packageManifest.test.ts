import fs from 'fs';
import path from 'path';

const root = path.join(__dirname, '..');
const pkg = JSON.parse(fs.readFileSync(path.join(root, 'package.json'), 'utf8'));
const lock = JSON.parse(fs.readFileSync(path.join(root, 'package-lock.json'), 'utf8'));

/**
 * What the shipped page needs is a dependency; tests, types and the compiler
 * are devDependencies (the release's licence list counts only the former).
 */
it('keeps test and type packages out of the runtime dependencies', () => {
  const runtime = Object.keys(pkg.dependencies);
  expect(runtime.filter((name) => /^@types\/|^@testing-library\/|^typescript$|jest/.test(name))).toEqual([]);
  expect(pkg.devDependencies).toHaveProperty('typescript');
  expect(pkg.devDependencies).toHaveProperty('@testing-library/react');
});

it('has a lock that agrees with the manifest', () => {
  expect(lock.name).toBe(pkg.name);
  expect(lock.packages[''].name).toBe(pkg.name);
  expect(lock.packages[''].version).toBe(pkg.version);
  expect(lock.packages[''].dependencies).toEqual(pkg.dependencies);
  expect(lock.packages[''].devDependencies).toEqual(pkg.devDependencies);
  for (const name of Object.keys(pkg.devDependencies)) {
    expect([name, lock.packages[`node_modules/${name}`]?.dev]).toEqual([name, true]);
  }
});

/**
 * guacamole-common-js is bundled from src/vendor (its package.json breaks
 * rspack), yet stays a dependency: the release's NOTICE and licence list are
 * built from package-lock.json, and the shipped Apache-2.0 code must be in them.
 */
it('keeps guacamole-common-js listed for the vendored copy', () => {
  expect(pkg.dependencies).toHaveProperty('guacamole-common-js');
  expect(fs.existsSync(path.join(__dirname, 'vendor', 'guacamole-common.js'))).toBe(true);
});

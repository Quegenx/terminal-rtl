import {mkdtempSync, readFileSync, rmSync, existsSync} from 'node:fs';
import {spawnSync} from 'node:child_process';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import assert from 'node:assert/strict';
import {validateBinary} from './check-package.mjs';
const repository = fileURLToPath(new URL('../..', import.meta.url));
const archive = resolve(process.argv[2]);
const root = mkdtempSync(join(tmpdir(), 'rtl-extracted-package-'));
function run(command, args, options = {}) {
  const result = spawnSync(command, args, {encoding:'utf8', timeout:120000, ...options});
  assert.equal(result.status, 0, result.stderr || String(result.error));
  return result.stdout;
}
try {
  const entries = run('tar', ['-tzf', archive]).trim().split('\n');
  assert.ok(entries.every(name => name.startsWith('package/') && !name.split('/').includes('..')), 'archive paths stay inside package/');
  run('tar', ['-xzf', archive, '-C', root]);
  const pkg = join(root, 'package');
  const {version} = JSON.parse(readFileSync(join(pkg,'package.json'),'utf8'));
  assert.ok(!existsSync(join(pkg,'target')));
  assert.ok(!existsSync(join(pkg,'rtl')));
  const target = `${process.platform}-${process.arch}`;
  const binary = join(pkg,'native',target,process.platform === 'win32' ? 'rtl.exe' : 'rtl');
  validateBinary(binary,target,version);
  assert.equal(run(process.execPath,[join(pkg,'bin/terminal-rtl.mjs'),'--version']).trim(),version);
  const env = {...process.env, RTL_CODEX_BIN:process.execPath, RTL_TEST_PACKAGE_ROOT:pkg};
  delete env.RTL_BIN;
  delete env.RTL_ACTIVE;
  const output = run(process.execPath,[join(pkg,'bin/terminal-rtl.mjs'),'codex','-e','console.log(JSON.stringify(process.argv.slice(1)))','--','שלום','space value'],{cwd:root,env});
  assert.deepEqual(JSON.parse(output),['שלום','space value']);
  const tests = run('cargo',['test','--locked','--test','pty','launchers::interactive_npm_launcher_uses_wrapper_and_preserves_exit_status','--','--exact'],{cwd:repository,env});
  assert.match(tests, /test result: ok\. 1 passed;/, 'the interactive package test must actually run');
  console.log(`Extracted ${target} package ${version} passed native, piped, and interactive launch checks.`);
} finally { rmSync(root,{recursive:true,force:true}); }

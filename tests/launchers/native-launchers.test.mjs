import {test} from 'node:test';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import {readFileSync, writeFileSync, mkdtempSync, rmSync, mkdirSync, existsSync} from 'node:fs';
import {join, dirname} from 'node:path';
import {tmpdir} from 'node:os';
import {resolveExecutable, powershellBatchArguments} from '../../bin/lib/agent-launcher.mjs';

test('native launchers forward arguments, cwd, and exit status without adding agent options', () => {
  for (const agent of ['codex', 'grok']) {
   for (const unified of [false, true]) {
    const script = fileURLToPath(new URL(unified ? '../../bin/terminal-rtl.mjs' : `../../bin/${agent}-rtl.mjs`, import.meta.url));
    const result = spawnSync(process.execPath, [script, ...(unified ? [agent] : []), '-e', 'console.log(JSON.stringify({args:process.argv.slice(1),cwd:process.cwd()}));process.exit(7)', '--', 'שלום', 'space value', '--model', 'unchanged'], {encoding:'utf8',timeout:10000,env:{...process.env,[`RTL_${agent.toUpperCase()}_BIN`]:process.execPath}});
    assert.equal(result.status,7,result.stderr);
    const output=JSON.parse(result.stdout);
    assert.deepEqual(output.args,['שלום','space value','--model','unchanged']);
    assert.equal(output.cwd,process.cwd());
   }
  }
});

test('unified launcher help, unknown agent and manifest version', () => {
  const script = fileURLToPath(new URL('../../bin/terminal-rtl.mjs', import.meta.url));
  const version = JSON.parse(readFileSync(new URL('../../package.json', import.meta.url), 'utf8')).version;
  for (const args of [[], ['--help'], ['-h']]) {
    const result = spawnSync(process.execPath, [script, ...args], {encoding:'utf8', timeout:10000});
    assert.equal(result.status, 0);
    assert.match(result.stdout, /Usage: terminal-rtl/);
  }
  assert.equal(spawnSync(process.execPath, [script, '--version'], {encoding:'utf8', timeout:10000}).stdout.trim(), version);
  assert.equal(spawnSync(process.execPath, [script, 'unknown'], {timeout:10000}).status, 1);
});

test('Windows resolution respects executable types and PATHEXT order', () => {
  const root = mkdtempSync(join(tmpdir(), 'rtl-resolve-'));
  try {
    for (const name of ['agent', 'agent.CMD', 'agent.EXE', 'agent.ps1']) writeFileSync(join(root, name), 'fixture');
    const options = {platform:'win32', path:root, pathext:'.CMD;.EXE;.PS1'};
    assert.equal(resolveExecutable('agent', options), join(root, 'agent.CMD'));
    assert.equal(resolveExecutable('agent', {...options, pathext:'.EXE;.CMD'}), join(root, 'agent.EXE'));
    assert.equal(resolveExecutable('agent.ps1', options), undefined);
    assert.equal(resolveExecutable('agent.CMD', options), join(root, 'agent.CMD'));
    const args = readFileSync(new URL('../fixtures/windows-arguments.txt', import.meta.url), 'utf8').trimEnd().split('\n');
    const command = "C:\\space folder\\O'Brien.cmd";
    const encoded = powershellBatchArguments(command, args).at(-1);
    const script = Buffer.from(encoded, 'base64').toString('utf16le');
    assert.ok(script.includes("& 'C:\\space folder\\O''Brien.cmd' "));
    assert.ok(script.includes("'' 'O''Brien' 'שלום' 'a&b'"));
  } finally { rmSync(root, {recursive:true, force:true}); }
});

test('Unix self-signaled children retain conventional signal exit status', {skip:process.platform === 'win32'}, () => {
  const script = fileURLToPath(new URL('../../bin/terminal-rtl.mjs', import.meta.url));
  const result = spawnSync(process.execPath, [script, 'codex', '-c', 'kill -TERM $$'], {
    env:{...process.env, RTL_CODEX_BIN:'/bin/sh'}, encoding:'utf8', timeout:10000,
  });
  assert.equal(result.status, 143, result.stderr);
});

test('real npm Windows shims launch through piped JS and interactive Rust paths', {skip:process.platform !== 'win32'}, () => {
  const root = mkdtempSync(join(tmpdir(), 'rtl-npm-shims-'));
  try {
    const fixture = join(root, 'fixture');
    const installed = join(root, 'installed');
    mkdirSync(fixture);
    writeFileSync(join(fixture,'package.json'),JSON.stringify({name:'rtl-audit-fixture',version:'1.0.0',bin:{codex:'cli.cjs'}}));
    writeFileSync(join(fixture,'cli.cjs'), `#!/usr/bin/env node
console.log('WINDOWS_ARGS_'+JSON.stringify(process.argv.slice(2))); if(process.stdin.isTTY){process.stdin.setRawMode(true);process.stdin.once('data',()=>process.exit(7));}else{process.exit(7);}
`);
    const node = resolveExecutable('node');
    const npm = join(dirname(node),'node_modules/npm/bin/npm-cli.js');
    const install = spawnSync(node,[npm,'install','--offline','--ignore-scripts','--no-audit','--no-fund','--prefix',installed,fixture],{encoding:'utf8',timeout:60000});
    assert.equal(install.status,0,install.stderr);
    const bin = join(installed,'node_modules/.bin');
    for (const name of ['codex','codex.cmd','codex.ps1']) assert.ok(existsSync(join(bin,name)));
    const env = {...process.env,PATH:`${bin};${process.env.PATH}`,RTL_TEST_WINDOWS_NPM_BIN:bin};
    delete env.RTL_CODEX_BIN;
    const args = ['שלום','space value',"O'Brien"];
    const launch = spawnSync(process.execPath,[fileURLToPath(new URL('../../bin/terminal-rtl.mjs',import.meta.url)),'codex',...args],{env,encoding:'utf8',timeout:10000});
    assert.equal(launch.status,7,launch.stderr);
    assert.deepEqual(JSON.parse(launch.stdout.trim().replace(/^WINDOWS_ARGS_/,'')),args);
    const interactive = spawnSync('cargo',['test','--locked','--test','pty','launchers::native_windows_npm_shim_interactive','--','--exact'],{cwd:new URL('../..',import.meta.url),env,encoding:'utf8',timeout:120000});
    assert.equal(interactive.status,0,interactive.stderr || String(interactive.error));
  } finally { rmSync(root,{recursive:true,force:true}); }
});

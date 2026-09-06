import {test} from 'node:test';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';

test('native launchers forward arguments, cwd, and exit status without adding agent options', () => {
  for (const agent of ['codex', 'grok']) {
    const script = fileURLToPath(new URL(`../bin/${agent}-rtl.mjs`, import.meta.url));
    const result = spawnSync(process.execPath, [script, '-e', 'console.log(JSON.stringify({args:process.argv.slice(1),cwd:process.cwd()}));process.exit(7)', '--', 'שלום', 'space value', '--model', 'unchanged'], {encoding:'utf8',env:{...process.env,[`RTL_${agent.toUpperCase()}_BIN`]:process.execPath}});
    assert.equal(result.status,7,result.stderr);
    const output=JSON.parse(result.stdout);
    assert.deepEqual(output.args,['שלום','space value','--model','unchanged']);
    assert.equal(output.cwd,process.cwd());
  }
});

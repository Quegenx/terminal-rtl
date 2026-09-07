import {test} from 'node:test';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {mkdtempSync, writeFileSync, rmSync} from 'node:fs';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {fileURLToPath} from 'node:url';

test('zsh helpers preserve aliases/functions, detect missing commands, and bypass nested wrapping', {skip:process.platform === 'win32'}, () => {
  const available = spawnSync('zsh',['--version'],{timeout:10000});
  // Linux source installations may not include zsh; CI installs it explicitly.
  if (available.error?.code === 'ENOENT') return;
  const root = mkdtempSync(join(tmpdir(),'rtl-shell-'));
  const helper = fileURLToPath(new URL('../../shell/rtl.zsh',import.meta.url));
  try {
    writeFileSync(join(root,'rtl'), '#!/bin/sh\nprintf "WRAPPED:%s\\n" "$*"\n',{mode:0o755});
    writeFileSync(join(root,'fixture-agent'), '#!/bin/sh\nprintf "DIRECT:%s\\n" "$*"\n',{mode:0o755});
    const run = code => spawnSync('zsh',['-f','-c','source "$1"; '+code,'zsh',helper],{env:{...process.env,PATH:`${root}:${process.env.PATH}`},encoding:'utf8',timeout:10000});
    for (const code of ["alias fixture-agent='echo ALIAS'; rtl-wrap fixture-agent", 'fixture-agent() { echo FUNCTION; }; rtl-wrap fixture-agent', 'rtl-wrap missing-fixture-command']) {
      assert.equal(run(code).status,1);
    }
    const normal = run('unset RTL_ACTIVE; rtl-wrap fixture-agent; fixture-agent שלום "space value"');
    assert.equal(normal.status,0,normal.stderr);
    assert.equal(normal.stdout.trim(),'WRAPPED:fixture-agent שלום space value');
    const nested = run('rtl-wrap fixture-agent; RTL_ACTIVE=1 fixture-agent שלום');
    assert.equal(nested.status,0,nested.stderr);
    assert.equal(nested.stdout.trim(),'DIRECT:שלום');
  } finally { rmSync(root,{recursive:true,force:true}); }
});

test('PowerShell source helpers protect existing commands and nested sessions', {skip:process.platform !== 'win32'}, () => {
  const result = spawnSync('powershell.exe',['-NoLogo','-NoProfile','-File',fileURLToPath(new URL('./helpers.ps1',import.meta.url))],{encoding:'utf8',timeout:10000});
  assert.equal(result.status,0,result.stderr);
});

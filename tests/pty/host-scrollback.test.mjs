import {test} from 'node:test';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {mkdtempSync, readFileSync, rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import xterm from '@xterm/headless';

test('resumed history reaches an independent host terminal scrollback', async () => {
  const directory = mkdtempSync(join(tmpdir(), 'terminal-rtl-host-'));
  const capture = join(directory, 'synthetic-output');
  const host = new xterm.Terminal({rows: 12, cols: 60, scrollback: 1000, allowProposedApi: true});
  const links = [];
  host.parser.registerOscHandler(8, data => { links.push(data); return false; });
  try {
    const result = spawnSync('cargo', ['test', '--locked', '--test', 'pty', 'history::inline_resume_uses_native_history_and_leaves_selection_to_the_terminal', '--', '--exact'], {
      cwd: new URL('../..', import.meta.url),
      env: {...process.env, RTL_TEST_HOST_CAPTURE: capture},
      encoding: 'utf8',
      timeout: 120000,
    });
    assert.equal(result.status, 0, result.stderr || String(result.error));
    await new Promise(resolve => host.write('EARLIER_SHELL_OUTPUT', resolve));
    await new Promise(resolve => host.write(readFileSync(capture), resolve));
    const buffer = host.buffer.active;
    assert.equal(buffer.type, 'normal');
    const lines = Array.from({length: buffer.length}, (_, i) => buffer.getLine(i).translateToString(true));
    const history = lines.slice(0, buffer.baseY).join('\n');
    const viewport = lines.slice(buffer.baseY).join('\n');
    assert.ok(history.includes('EARLIER_SHELL_OUTPUT'), 'keep preexisting shell history');
    assert.ok(history.includes('RESUMED_000'), 'oldest resumed row must be in host scrollback');
    assert.ok(history.includes('RESUMED_040'), 'later resumed rows must reach host scrollback');
    assert.ok(viewport.includes('INLINE_DRAFT_READY'), 'restore composer');
    assert.ok(viewport.includes('Powered by: Gal Havkin'), 'restore footer');
    assert.ok(links.some(link => link.endsWith(';https://example.com/complete/destination')), 'preserve complete hyperlink destinations');
    assert.equal(links.at(-1), ';', 'close link before the composer and footer');
  } finally {
    host.dispose();
    rmSync(directory, {recursive: true, force: true});
  }
});

test('independent host keeps earlier history across child clears, bursts and RIS', async () => {
  const directory = mkdtempSync(join(tmpdir(), 'terminal-rtl-burst-'));
  const capture = join(directory, 'output');
  const host = new xterm.Terminal({rows:12, cols:60, scrollback:1000, allowProposedApi:true});
  try {
    const result = spawnSync('cargo', ['test','--locked','--test','pty','history::inline_bursts_and_child_clears_preserve_host_history','--','--exact'], {cwd:new URL('../..',import.meta.url), env:{...process.env,RTL_TEST_BURST_CAPTURE:capture}, encoding:'utf8', timeout:120000});
    assert.equal(result.status,0,result.stderr || String(result.error));
    await new Promise(resolve => host.write('EARLIER_SHELL_OUTPUT',resolve));
    await new Promise(resolve => host.write(readFileSync(capture),resolve));
    const buffer = host.buffer.active;
    const history = Array.from({length:buffer.baseY},(_,i) => buffer.getLine(i).translateToString(true)).join('\n');
    assert.ok(history.includes('EARLIER_SHELL_OUTPUT'));
    for (let i=0;i<90;i++) assert.ok(history.includes(`BURST_${String(i).padStart(3,'0')}`), `missing ${i}`);
    assert.ok(buffer.getLine(buffer.baseY + 10).translateToString(true).includes('BURST_READY'));
  } finally { host.dispose(); rmSync(directory,{recursive:true,force:true}); }
});

test('independent host checks replay links, clipped wide history, origin queries and reset', async () => {
  const directory = mkdtempSync(join(tmpdir(), 'terminal-rtl-audit-host-'));
  const hosts = [];
  const create = (rows,cols) => { const host = new xterm.Terminal({rows,cols,scrollback:100,allowProposedApi:true}); hosts.push(host); return host; };
  try {
    const result = spawnSync('cargo',['test','--locked','--test','audit','independent_host_fixtures','--','--exact'],{cwd:new URL('../..',import.meta.url),env:{...process.env,RTL_TEST_AUDIT_CAPTURE:directory},encoding:'utf8',timeout:120000});
    assert.equal(result.status,0,result.stderr || String(result.error));
    const replay = create(8,20);
    const links = [];
    replay.parser.registerOscHandler(8,data => { links.push(data); return false; });
    await new Promise(resolve => replay.write(readFileSync(join(directory,'replay')),resolve));
    assert.equal(replay.buffer.active.getLine(0).translateToString(true),'ABCDEFGH');
    assert.ok(links.some(link => link.endsWith(';https://example.com/' + 'p;'.repeat(30) + 'end')));
    assert.equal(links.at(-1),';');
    const geometry = create(2,3);
    await new Promise(resolve => geometry.write(readFileSync(join(directory,'geometry')),resolve));
    assert.equal(geometry.buffer.active.getLine(0).translateToString(true),'ab');
    assert.equal(geometry.buffer.active.getLine(1).translateToString(true),'nex');
    const modes = create(4,8);
    const replies = [];
    modes.onData(data => replies.push(data));
    await new Promise(resolve => modes.write(readFileSync(join(directory,'modes-input')),resolve));
    // xterm.js 6 reports absolute CPR even with DECOM; Terminal RTL follows
    // DEC's origin-relative policy, covered by the focused Rust regression.
    assert.deepEqual(replies,['\x1b[2;1R','\x1b[1;1R']);
    assert.equal(readFileSync(join(directory,'modes-replies'),'utf8'),replies.at(-1));
    assert.equal(modes.buffer.active.cursorX,0);
    assert.equal(modes.buffer.active.cursorY,0);
  } finally { hosts.forEach(host => host.dispose()); rmSync(directory,{recursive:true,force:true}); }
});

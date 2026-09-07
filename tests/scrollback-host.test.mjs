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
    const result = spawnSync('cargo', ['test', '--locked', '--test', 'pty', 'inline_resume_uses_native_history_and_leaves_selection_to_the_terminal', '--', '--exact'], {
      cwd: new URL('..', import.meta.url),
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

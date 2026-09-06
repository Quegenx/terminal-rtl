import {spawn} from 'node:child_process';
import {existsSync} from 'node:fs';
import {dirname, resolve, extname} from 'node:path';
import {homedir} from 'node:os';
import {fileURLToPath} from 'node:url';

/** Launch the installed CLI itself: no app-server, model override, or injected tools. */
export function launchNativeAgent(agent) {
  const args = process.argv.slice(2);
  const env = {...process.env};
  const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
  const override = env[agent === 'codex' ? 'RTL_CODEX_BIN' : 'RTL_GROK_BIN'];
  const fallback = resolve(homedir(), `.${agent}/bin/${agent}${process.platform === 'win32' ? '.exe' : ''}`);
  const executable = override || Bun.which(agent) || (existsSync(fallback) ? fallback : agent);
  const local = resolve(root, 'target/release', process.platform === 'win32' ? 'rtl.exe' : 'rtl');
  const packaged = resolve(root, process.platform === 'win32' ? 'rtl.exe' : 'rtl');
  const wrapper = env.RTL_BIN || (existsSync(local) ? local : existsSync(packaged) ? packaged : 'rtl');
  const interactive = process.stdin.isTTY && process.stdout.isTTY && !env.RTL_ACTIVE;
  let command = interactive ? wrapper : executable;
  let parameters = interactive ? [...(env.RTL_PRETTY === '0' ? [] : ['--pretty', '--agent-label', agent]), '--', executable, ...args] : args;
  if (!interactive && process.platform === 'win32' && /\.(cmd|bat)$/i.test(extname(command))) {
    const quote = value => "'" + value.replaceAll("'", "''") + "'";
    const script = '& ' + [command, ...parameters].map(quote).join(' ') + '; exit $LASTEXITCODE';
    command = 'powershell.exe';
    parameters = ['-NoLogo', '-NoProfile', '-EncodedCommand', Buffer.from(script, 'utf16le').toString('base64')];
  }
  const child = spawn(command, parameters, {stdio: 'inherit', env});
  child.on('error', error => { console.error(`${agent}-rtl: ${error.message}`); process.exitCode = 1; });
  child.on('exit', (code, signal) => { process.exitCode = code ?? (signal ? 1 : 0); });
  process.on('SIGTERM', () => child.kill('SIGTERM'));
}

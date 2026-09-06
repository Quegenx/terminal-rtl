import {spawn} from 'node:child_process';
import {existsSync, accessSync, constants} from 'node:fs';
import {dirname, resolve, extname, delimiter} from 'node:path';
import {homedir} from 'node:os';
import {fileURLToPath} from 'node:url';

function which(name) {
  const extensions = process.platform === 'win32' ? ['', ...(process.env.PATHEXT || '.EXE;.CMD;.BAT').split(';')] : [''];
  for (const directory of (process.env.PATH || '').split(delimiter).filter(Boolean)) {
    for (const extension of extensions) {
      const candidate = resolve(directory, name + extension);
      try { accessSync(candidate, constants.X_OK); return candidate; } catch {}
    }
  }
}

/** Launch the installed CLI itself: no app-server, model override, or injected tools. */
export function launchNativeAgent(agent, args = process.argv.slice(2)) {
  const env = {...process.env};
  const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
  const override = env[agent === 'codex' ? 'RTL_CODEX_BIN' : 'RTL_GROK_BIN'];
  const fallback = resolve(homedir(), `.${agent}/bin/${agent}${process.platform === 'win32' ? '.exe' : ''}`);
  const executable = override || which(agent) || (existsSync(fallback) ? fallback : agent);
  const local = resolve(root, 'target/release', process.platform === 'win32' ? 'rtl.exe' : 'rtl');
  const packaged = resolve(root, process.platform === 'win32' ? 'rtl.exe' : 'rtl');
  const bundled = resolve(root, 'native', `${process.platform}-${process.arch}`, process.platform === 'win32' ? 'rtl.exe' : 'rtl');
  const wrapper = env.RTL_BIN || [local, packaged, bundled].find(existsSync) || which('rtl');
  const interactive = process.stdin.isTTY && process.stdout.isTTY && !env.RTL_ACTIVE;
  if (interactive && !wrapper) {
    console.error(`terminal-rtl: no native wrapper for ${process.platform}-${process.arch}. Set RTL_BIN to a compiled rtl executable.`);
    process.exitCode = 1;
    return;
  }
  let command = interactive ? wrapper : executable;
  let parameters = interactive ? ['--attribution', ...(env.RTL_PRETTY === '0' ? [] : ['--pretty', '--agent-label', agent]), '--', executable, ...args] : args;
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

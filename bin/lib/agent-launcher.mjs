import {spawn} from 'node:child_process';
import {existsSync, accessSync, constants, statSync} from 'node:fs';
import {resolve, extname, delimiter} from 'node:path';
import {homedir, constants as osConstants} from 'node:os';
import {fileURLToPath} from 'node:url';

export function resolveExecutable(name, {platform = process.platform, path = process.env.PATH || '', pathext = process.env.PATHEXT || '.COM;.EXE;.BAT;.CMD'} = {}) {
  const windows = platform === 'win32';
  const supported = extension => /^\.(com|exe|bat|cmd)$/i.test(extension);
  const extensions = windows
    ? (extname(name) ? (supported(extname(name)) ? [''] : []) : pathext.split(';').filter(supported))
    : [''];
  const directories = /[/\\]/.test(name) ? [''] : [...(windows ? ['.'] : []), ...path.split(windows ? ';' : delimiter).filter(Boolean)];
  for (const directory of directories) {
    for (const extension of extensions) {
      const candidate = resolve(directory, name + extension);
      try {
        if (!statSync(candidate).isFile()) continue;
        accessSync(candidate, windows ? constants.F_OK : constants.X_OK);
        return candidate;
      } catch {}
    }
  }
}

export function powershellBatchArguments(command, parameters) {
  const quote = value => "'" + value.replaceAll("'", "''") + "'";
  const script = "$ErrorActionPreference = 'Stop'; [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false); $OutputEncoding = [Console]::OutputEncoding; & " + [command, ...parameters].map(quote).join(' ') + '; if ($null -ne $LASTEXITCODE) { exit $LASTEXITCODE } else { exit 1 }';
  return ['-NoLogo', '-NoProfile', '-EncodedCommand', Buffer.from(script, 'utf16le').toString('base64')];
}

/** Launch the installed CLI itself: no app-server, model override, or injected tools. */
export function launchNativeAgent(agent, args = process.argv.slice(2)) {
  const env = {...process.env};
  const root = fileURLToPath(new URL('../..', import.meta.url));
  const override = env[agent === 'codex' ? 'RTL_CODEX_BIN' : 'RTL_GROK_BIN'];
  const fallback = resolve(homedir(), `.${agent}/bin/${agent}${process.platform === 'win32' ? '.exe' : ''}`);
  const executable = override || resolveExecutable(agent) || (existsSync(fallback) ? fallback : agent);
  const local = resolve(root, 'target/release', process.platform === 'win32' ? 'rtl.exe' : 'rtl');
  const packaged = resolve(root, process.platform === 'win32' ? 'rtl.exe' : 'rtl');
  const bundled = resolve(root, 'native', `${process.platform}-${process.arch}`, process.platform === 'win32' ? 'rtl.exe' : 'rtl');
  const wrapper = env.RTL_BIN || [local, packaged, bundled].find(existsSync) || resolveExecutable('rtl');
  const interactive = process.stdin.isTTY && process.stdout.isTTY && !env.RTL_ACTIVE;
  if (interactive && !wrapper) {
    console.error(`terminal-rtl: no bundled RTL engine for ${process.platform}-${process.arch}. This package currently bundles macOS Apple Silicon and Windows x64.`);
    process.exitCode = 1;
    return;
  }
  let command = interactive ? wrapper : executable;
  let parameters = interactive ? ['--inline', '--attribution', ...(env.RTL_PRETTY === '0' ? [] : ['--pretty', '--agent-label', agent]), '--', executable, ...args] : args;
  if (!interactive && process.platform === 'win32' && /\.(cmd|bat)$/i.test(extname(command))) {
    parameters = powershellBatchArguments(command, parameters);
    command = 'powershell.exe';
  }
  const child = spawn(command, parameters, {stdio: 'inherit', env});
  const forwardTermination = () => child.kill('SIGTERM');
  process.on('SIGTERM', forwardTermination);
  child.on('error', error => {
    process.removeListener('SIGTERM', forwardTermination);
    console.error(`${agent}-rtl: ${error.message}`);
    process.exitCode = 1;
  });
  child.on('exit', (code, signal) => {
    process.removeListener('SIGTERM', forwardTermination);
    process.exitCode = code ?? (signal ? 128 + (osConstants.signals[signal] || 0) : 0);
  });
}

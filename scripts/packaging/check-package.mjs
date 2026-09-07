import {lstatSync, readFileSync, accessSync, constants} from 'node:fs';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import {resolve, join} from 'node:path';

export function validateBinary(path, platform, version, {run = true} = {}) {
  const stat = lstatSync(path);
  if (!stat.isFile()) throw new Error(`Not a regular binary: ${path}`);
  const bytes = readFileSync(path);
  if (bytes.length < 1024) throw new Error(`Incomplete binary: ${path}`);
  if (platform === 'darwin-arm64') {
    if (bytes.readUInt32LE(0) !== 0xfeedfacf || bytes.readUInt32LE(4) !== 0x0100000c || bytes.readUInt32LE(12) !== 2) {
      throw new Error(`Expected an arm64 Mach-O executable: ${path}`);
    }
    if (process.platform !== 'win32') accessSync(path, constants.X_OK);
    if (!(stat.mode & 0o111) && process.platform !== 'win32') throw new Error(`Binary is not executable: ${path}`);
  } else if (platform === 'win32-x64') {
    const pe = bytes.readUInt32LE(0x3c);
    if (bytes.toString('ascii', 0, 2) !== 'MZ' || pe < 64 || pe + 94 > bytes.length ||
        bytes.readUInt32LE(pe) !== 0x4550 || bytes.readUInt16LE(pe + 4) !== 0x8664 ||
        !(bytes.readUInt16LE(pe + 22) & 2) || bytes.readUInt16LE(pe + 24) !== 0x20b ||
        bytes.readUInt16LE(pe + 92) !== 3) {
      throw new Error(`Expected an x64 PE console executable: ${path}`);
    }
  } else throw new Error(`Unsupported package target: ${platform}`);
  const markers = [...bytes.toString('latin1').matchAll(/terminal-rtl-version:([^:]+):end/g)].map(match => match[1]);
  if (!markers.length || markers.some(marker => marker !== version)) throw new Error(`Binary version does not match ${version}: ${path}`);
  if (run && platform === `${process.platform}-${process.arch}`) {
    const result = spawnSync(path, ['--version'], {encoding: 'utf8', timeout: 10000});
    if (result.status !== 0 || result.stdout.trim() !== `rtl ${version}`) throw new Error(`Native version smoke test failed: ${path}`);
  }
}

export function checkPackage(root = fileURLToPath(new URL('../..', import.meta.url))) {
  const {version} = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'));
  const cargo = readFileSync(join(root, 'Cargo.toml'), 'utf8');
  const cargoVersion = cargo.split(/\n\[/)[0].match(/^version\s*=\s*"([^"]+)"/m)?.[1];
  if (cargoVersion !== version) throw new Error('Cargo and npm versions differ');
  for (const [platform, name] of [['darwin-arm64', 'rtl'], ['win32-x64', 'rtl.exe']]) {
    validateBinary(join(root, 'native', platform, name), platform, version);
  }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) checkPackage();

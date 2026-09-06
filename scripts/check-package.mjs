import {statSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
for (const binary of ['darwin-arm64/rtl', 'win32-x64/rtl.exe']) {
  const path = fileURLToPath(new URL(`../native/${binary}`, import.meta.url));
  try {
    if (statSync(path).size < 1000) throw new Error('empty binary');
  } catch {
    throw new Error(`Missing release binary: ${path}. Build the native binaries or download the npm-package artifact from GitHub Actions.`);
  }
}

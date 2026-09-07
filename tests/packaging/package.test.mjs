import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtempSync, writeFileSync, chmodSync, mkdirSync, rmSync} from 'node:fs';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {validateBinary, checkPackage} from '../../scripts/packaging/check-package.mjs';

function fixture(platform, version = '0.1.5') {
  const bytes = Buffer.alloc(2048);
  if (platform === 'darwin-arm64') {
    bytes.writeUInt32LE(0xfeedfacf, 0); bytes.writeUInt32LE(0x0100000c, 4); bytes.writeUInt32LE(2, 12);
  } else {
    bytes.write('MZ'); bytes.writeUInt32LE(128, 0x3c); bytes.writeUInt32LE(0x4550, 128);
    bytes.writeUInt16LE(0x8664, 132); bytes.writeUInt16LE(2, 150); bytes.writeUInt16LE(0x20b, 152); bytes.writeUInt16LE(3, 220);
  }
  bytes.write(`terminal-rtl-version:${version}:end`, 512);
  return bytes;
}

test('package checker rejects malformed, wrong-architecture, stale and non-executable binaries', () => {
  const root = mkdtempSync(join(tmpdir(), 'rtl-package-test-'));
  try {
    const path = join(root, 'rtl');
    assert.throws(() => validateBinary(path, 'darwin-arm64', '0.1.5'));
    mkdirSync(path);
    assert.throws(() => validateBinary(path, 'darwin-arm64', '0.1.5'), /regular/);
    rmSync(path, {recursive:true});
    for (const platform of ['darwin-arm64', 'win32-x64']) {
      writeFileSync(path, 'x'.repeat(1001), {mode:0o755});
      assert.throws(() => validateBinary(path, platform, '0.1.5'));
      const wrong = fixture(platform);
      if (platform === 'darwin-arm64') wrong.writeUInt32LE(0x01000007, 4);
      else wrong.writeUInt16LE(0x14c, 132);
      writeFileSync(path, wrong);
      assert.throws(() => validateBinary(path, platform, '0.1.5'), /executable/);
      writeFileSync(path, fixture(platform, '0.0.1'));
      assert.throws(() => validateBinary(path, platform, '0.1.5'), /version/);
      writeFileSync(path, fixture(platform));
      validateBinary(path, platform, '0.1.5', {run:false});
    }
    if (process.platform !== 'win32') {
      writeFileSync(path, fixture('darwin-arm64'));
      chmodSync(path, 0o644);
      assert.throws(() => validateBinary(path, 'darwin-arm64', '0.1.5'));
    }
    writeFileSync(join(root, 'package.json'), JSON.stringify({version:'1.0.0'}));
    writeFileSync(join(root, 'Cargo.toml'), '[package]\nversion = "2.0.0"\n');
    assert.throws(() => checkPackage(root), /versions differ/);
  } finally { rmSync(root, {recursive:true, force:true}); }
});

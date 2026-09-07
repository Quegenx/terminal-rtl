import {readdirSync} from 'node:fs';
import {spawnSync} from 'node:child_process';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';

const root = fileURLToPath(new URL('../..', import.meta.url));
function testFiles(directory) {
  return readdirSync(join(root, directory), {withFileTypes:true}).flatMap(entry => {
    const path = join(directory, entry.name);
    return entry.isDirectory() ? testFiles(path) : entry.name.endsWith('.test.mjs') ? [path] : [];
  }).sort();
}
const files = testFiles('tests');
// Node 18.0 has node:test but predates the --test CLI switch (18.1).
const [major, minor] = process.versions.node.split('.').map(Number);
const invocations = major === 18 && minor === 0 ? files.map(file => [file]) : [['--test', ...files]];
for (const args of invocations) {
  const result = spawnSync(process.execPath, args, {cwd:root, stdio:'inherit', timeout:180000});
  if (result.error) console.error(result.error.message);
  if (result.status !== 0) { process.exitCode = result.status ?? 1; break; }
}

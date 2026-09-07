import {readdirSync, readFileSync} from 'node:fs';
import {extname, join, relative} from 'node:path';
import {fileURLToPath} from 'node:url';

const root = fileURLToPath(new URL('../..', import.meta.url));
const limit = 300;
// Upstream and generated layouts are deliberately outside the first-party limit.
const excluded = new Set(['.git', 'vendor', 'target', 'native', 'dist', 'node_modules']);
const extensions = new Set(['.rs', '.js', '.mjs', '.cjs', '.ts', '.tsx', '.jsx', '.md', '.ps1', '.sh', '.zsh', '.yml', '.yaml', '.json', '.toml', '.html', '.css', '.sql', '.txt']);
const configuration = new Set(['.editorconfig', '.gitignore', 'LICENSE']);
let checked = 0;
const failures = [];
function inspect(directory) {
  for (const entry of readdirSync(directory, {withFileTypes:true})) {
    if (directory === root && (excluded.has(entry.name) || entry.name === 'THIRD_PARTY_LICENSES.txt')) continue;
    const path = join(directory, entry.name);
    if (entry.isDirectory()) inspect(path);
    else if (entry.isFile() && (extensions.has(extname(entry.name)) || configuration.has(entry.name))) {
      const contents = readFileSync(path, 'utf8');
      const lines = contents ? contents.split('\n').length - Number(contents.endsWith('\n')) : 0;
      checked++;
      if (lines > limit) failures.push(`${relative(root, path)}: ${lines} lines (maximum ${limit})`);
    }
  }
}
inspect(root);
if (failures.length) {
  console.error(failures.join('\n'));
  process.exitCode = 1;
} else console.log(`Structure check passed: ${checked} first-party files, at most ${limit} lines each.`);

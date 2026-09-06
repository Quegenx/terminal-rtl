#!/usr/bin/env node
import {launchNativeAgent} from './native-launchers.mjs';
const [agent, ...args] = process.argv.slice(2);
if (agent === 'codex' || agent === 'grok') {
  launchNativeAgent(agent, args);
} else if (!agent || agent === '--help' || agent === '-h') {
  console.log('Terminal RTL\nPowered by: Gal Havkin\nUsage: terminal-rtl <codex|grok> [native CLI arguments]');
} else {
  console.error(`terminal-rtl: unknown agent ${agent}; use codex or grok.`);
  process.exitCode = 1;
}

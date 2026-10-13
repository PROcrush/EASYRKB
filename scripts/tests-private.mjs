// Runs test suites in rbxport-private that require the private
// AlphaTheta Emulator and physical hardware on the network.
//
// npm run tests:private
// npm run tests:private -- cdj-3000
// npm run tests:private -- xdj-az

import { existsSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const repo = path.resolve(fileURLToPath(new URL('../', import.meta.url)));
const privateRepo = path.resolve(process.env.RBXPORT_PRIVATE_REPO || path.join(repo, '../rbxport-private'));
const suites = {
  'cdj-3000': 'scripts/e2e-link/run.sh',
  'xdj-az': 'scripts/e2e-xdj-az/run.sh',
};
const requested = process.argv.slice(2);
if (requested.includes('--help')) {
  console.log('Usage: npm run tests:private -- [cdj-3000] [xdj-az]\nDefaults to both firmware suites. Requires rbxport-private and built AtEmu firmware models.\nSet RBXPORT_PRIVATE_REPO and ATEMU_DIR to override checkout locations.');
  process.exit(0);
}
const selected = requested.length ? requested : Object.keys(suites);
for (const name of selected) {
  if (!Object.hasOwn(suites, name)) {
    console.error(`Unknown private suite: ${name}. Choose cdj-3000 or xdj-az.`);
    process.exit(2);
  }
  const script = path.join(privateRepo, suites[name]);
  if (!existsSync(script)) {
    console.error(`Missing private harness: ${script}\nCheck out rbxport-private beside RBXport or set RBXPORT_PRIVATE_REPO.`);
    process.exit(2);
  }
}
for (const name of selected) {
  console.log(`Running private firmware suite: ${name}`);
  const result = spawnSync('bash', [path.join(privateRepo, suites[name])], {
    cwd: privateRepo,
    env: { ...process.env, RBXPORT_REPO: repo },
    stdio: 'inherit',
  });
  if (result.error) console.error(result.error.message);
  if (result.status !== 0) process.exit(result.status ?? 1);
}

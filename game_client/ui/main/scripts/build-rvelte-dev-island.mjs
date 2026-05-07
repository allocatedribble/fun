import { copyFileSync, cpSync, mkdirSync, rmSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDir = dirname(fileURLToPath(import.meta.url));
const uiRoot = resolve(scriptDir, '..');
const projectRoot = resolve(uiRoot, '../../../..');
const source = join(projectRoot, 'rvelte/examples/browser-dev-panel/dist');
const target = join(uiRoot, 'public/rvelte-dev');

rmSync(target, { recursive: true, force: true });
mkdirSync(join(target, 'generated'), { recursive: true });

copyFileSync(join(source, 'app.js'), join(target, 'dev-panel.js'));
copyFileSync(join(source, 'dev_panel.wasm'), join(target, 'dev_panel.wasm'));
cpSync(join(source, 'generated'), join(target, 'generated'), { recursive: true });
copyFileSync(
  join(uiRoot, 'public/rvelte-dev.README.template.md'),
  join(target, 'README.md')
);

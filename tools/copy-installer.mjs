// Runs after `npm run build` (npm's "postbuild" hook): copies the newest
// NSIS installer from Tauri's bundle folder to ./installer/ at the project
// root, replacing older Courier installers there. ./installer/ is git-ignored.
import { copyFileSync, mkdirSync, readdirSync, rmSync, statSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const from = join(root, "src-tauri", "target", "release", "bundle", "nsis");
const to = join(root, "installer");
const isInstaller = (f) => /^Courier_.+-setup\.exe$/.test(f);

const newest = readdirSync(from)
  .filter(isInstaller)
  .map((f) => ({ f, t: statSync(join(from, f)).mtimeMs }))
  .sort((a, b) => b.t - a.t)[0];
if (!newest) {
  console.error(`No installer found in ${from}`);
  process.exit(1);
}

mkdirSync(to, { recursive: true });
for (const f of readdirSync(to)) if (isInstaller(f) && f !== newest.f) rmSync(join(to, f));
copyFileSync(join(from, newest.f), join(to, newest.f));
console.log(`Installer copied to ${join(to, newest.f)}`);

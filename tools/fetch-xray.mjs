// Downloads the Xray-core release named in tools/xray.version for one platform, checks its
// SHA-256 against the published digest and unpacks what the app needs into
// src-tauri/resources/xray.
//   node tools/fetch-xray.mjs [--target <rust target triple>]
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { chmodSync, cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const version = readFileSync(join(root, 'tools', 'xray.version'), 'utf8').trim();
const flag = process.argv.indexOf('--target');
const triple = flag > 0 ? process.argv[flag + 1] : '';

const os = triple ? (triple.includes('windows') ? 'win32' : triple.includes('apple') ? 'darwin' : 'linux') : process.platform;
const arch = triple ? (triple.startsWith('aarch64') ? 'arm64' : 'x64') : process.arch;
const asset = {
  'win32-x64': 'Xray-windows-64.zip',
  'win32-arm64': 'Xray-windows-arm64-v8a.zip',
  'linux-x64': 'Xray-linux-64.zip',
  'linux-arm64': 'Xray-linux-arm64-v8a.zip',
  'darwin-x64': 'Xray-macos-64.zip',
  'darwin-arm64': 'Xray-macos-arm64-v8a.zip',
}[`${os}-${arch}`];
if (!asset) throw new Error(`no Xray build for ${os}-${arch}`);

const url = `https://github.com/XTLS/Xray-core/releases/download/${version}/${asset}`;
const get = async (u) => {
  const r = await fetch(u, { redirect: 'follow' });
  if (!r.ok) throw new Error(`${u}: ${r.status}`);
  return Buffer.from(await r.arrayBuffer());
};

console.log(`Xray ${version} for ${os}-${arch}: ${asset}`);
const zip = await get(url);
const digest = (await get(`${url}.dgst`)).toString('utf8');
const expected = /SHA2-256=\s*([0-9a-f]{64})/i.exec(digest)?.[1]?.toLowerCase();
const actual = createHash('sha256').update(zip).digest('hex');
if (!expected || expected !== actual) throw new Error(`checksum mismatch: expected ${expected}, got ${actual}`);
console.log('sha256 ok', actual);

const work = mkdtempSync(join(tmpdir(), 'xray-'));
const archive = join(work, asset);
writeFileSync(archive, zip);
if (process.platform === 'win32') {
  execFileSync('powershell', ['-NoProfile', '-Command', `Expand-Archive -LiteralPath '${archive}' -DestinationPath '${work}' -Force`]);
} else {
  execFileSync('unzip', ['-o', '-q', archive, '-d', work]);
}

const dest = join(root, 'src-tauri', 'resources', 'xray');
rmSync(dest, { recursive: true, force: true });
mkdirSync(dest, { recursive: true });
const binary = os === 'win32' ? 'xray.exe' : 'xray';
const wanted = [binary, 'geoip.dat', 'geosite.dat', 'LICENSE', ...(os === 'win32' ? ['wintun.dll'] : [])];
for (const name of wanted) {
  if (!existsSync(join(work, name))) throw new Error(`${name} is missing from ${asset}`);
  cpSync(join(work, name), join(dest, name));
}
// the wintun licence ships under different names in different releases
const wintunLicence = readdirSync(work).find((n) => /^license.*wintun/i.test(n));
if (os === 'win32' && wintunLicence) cpSync(join(work, wintunLicence), join(dest, 'LICENSE-wintun.txt'));
if (os !== 'win32') chmodSync(join(dest, binary), 0o755);
rmSync(work, { recursive: true, force: true });
console.log('unpacked to', dest);

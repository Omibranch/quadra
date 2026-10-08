// Run after `tauri android init`: adjusts the generated Gradle project for what Quadra needs.
//   node tools/android-prepare.mjs
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const gradle = join(root, 'src-tauri', 'gen', 'android', 'app', 'build.gradle.kts');
let text = readFileSync(gradle, 'utf8');

// The core is started as a program, and Android only runs programs that exist as real files in
// the package's native library folder. By default native libraries stay inside the APK.
if (!text.includes('useLegacyPackaging')) {
  const at = text.indexOf('android {');
  if (at < 0) throw new Error('no android { } block in ' + gradle);
  const insert = '\n    packaging {\n        jniLibs {\n            useLegacyPackaging = true\n        }\n    }';
  text = text.slice(0, at + 'android {'.length) + insert + text.slice(at + 'android {'.length);
  writeFileSync(gradle, text);
  console.log('legacy native-library packaging enabled');
}

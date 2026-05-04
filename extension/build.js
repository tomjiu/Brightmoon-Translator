// Build script for Moon Translator Browser Extension
// Creates Chrome and Firefox versions

const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

const ROOT_DIR = __dirname;
const DIST_DIR = path.join(ROOT_DIR, 'dist');

// Clean dist
if (fs.existsSync(DIST_DIR)) {
  fs.rmSync(DIST_DIR, { recursive: true });
}
fs.mkdirSync(DIST_DIR, { recursive: true });

// Chrome version
const chromeDir = path.join(DIST_DIR, 'chrome');
fs.mkdirSync(chromeDir, { recursive: true });
fs.mkdirSync(path.join(chromeDir, 'background'), { recursive: true });
fs.mkdirSync(path.join(chromeDir, 'content'), { recursive: true });
fs.mkdirSync(path.join(chromeDir, 'popup'), { recursive: true });
fs.mkdirSync(path.join(chromeDir, 'icons'), { recursive: true });

// Copy Chrome files
copyDir(ROOT_DIR, chromeDir, ['build.js', 'dist', 'README.md', 'generate.html']);

// Firefox version (MV2 compatible)
const firefoxDir = path.join(DIST_DIR, 'firefox');
fs.mkdirSync(firefoxDir, { recursive: true });
fs.mkdirSync(path.join(firefoxDir, 'background'), { recursive: true });
fs.mkdirSync(path.join(firefoxDir, 'content'), { recursive: true });
fs.mkdirSync(path.join(firefoxDir, 'popup'), { recursive: true });
fs.mkdirSync(path.join(firefoxDir, 'icons'), { recursive: true });

// Copy Firefox files
copyDir(ROOT_DIR, firefoxDir, ['build.js', 'dist', 'README.md', 'generate.html']);

// Modify Firefox manifest
const firefoxManifest = JSON.parse(fs.readFileSync(path.join(ROOT_DIR, 'manifest.json'), 'utf8'));
firefoxManifest.manifest_version = 2;
firefoxManifest.background = {
  scripts: ['background/service-worker.js']
};
firefoxManifest.browser_action = firefoxManifest.action;
delete firefoxManifest.action;
firefoxManifest.permissions = firefoxManifest.permissions.filter(p => p !== 'scripting');
fs.writeFileSync(path.join(firefoxDir, 'manifest.json'), JSON.stringify(firefoxManifest, null, 2));

// Create zip files
console.log('Creating Chrome extension...');
execSync(`cd "${chromeDir}" && tar -cf ../moontranslator-chrome.zip .`, { stdio: 'inherit' });

console.log('Creating Firefox extension...');
execSync(`cd "${firefoxDir}" && tar -cf ../moontranslator-firefox.zip .`, { stdio: 'inherit' });

console.log('\nBuild complete!');
console.log(`Chrome: ${path.join(DIST_DIR, 'moontranslator-chrome.zip')}`);
console.log(`Firefox: ${path.join(DIST_DIR, 'moontranslator-firefox.zip')}`);

// Helper: Copy directory
function copyDir(src, dest, exclude = []) {
  const entries = fs.readdirSync(src, { withFileTypes: true });

  for (const entry of entries) {
    if (exclude.includes(entry.name)) continue;

    const srcPath = path.join(src, entry.name);
    const destPath = path.join(dest, entry.name);

    if (entry.isDirectory()) {
      fs.mkdirSync(destPath, { recursive: true });
      copyDir(srcPath, destPath, exclude);
    } else {
      fs.copyFileSync(srcPath, destPath);
    }
  }
}

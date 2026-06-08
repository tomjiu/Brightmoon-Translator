// Build script for Moon Translator Browser Extension.
// Creates Chrome MV3 and Firefox MV2 packages.

import { execSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT_DIR = path.dirname(fileURLToPath(import.meta.url));
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
delete firefoxManifest.commands;

// Convert web_accessible_resources from MV3 object format to MV2 string array format
if (firefoxManifest.web_accessible_resources && Array.isArray(firefoxManifest.web_accessible_resources)) {
  const resources = [];
  for (const entry of firefoxManifest.web_accessible_resources) {
    if (entry.resources) {
      resources.push(...entry.resources);
    }
  }
  firefoxManifest.web_accessible_resources = resources;
}

fs.writeFileSync(path.join(firefoxDir, 'manifest.json'), JSON.stringify(firefoxManifest, null, 2));

// Create portable archives. For local development, load the unpacked dist folders.
console.log('Creating Chrome extension...');
archiveDir(chromeDir, path.join(DIST_DIR, 'moontranslator-chrome.tar.gz'));

console.log('Creating Firefox extension...');
archiveDir(firefoxDir, path.join(DIST_DIR, 'moontranslator-firefox.tar.gz'));

console.log('\nBuild complete!');
console.log(`Chrome directory: ${chromeDir}`);
console.log(`Firefox directory: ${firefoxDir}`);

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

function archiveDir(src, dest) {
  try {
    // Try creating a zip archive (more portable on Windows)
    const zipDest = dest.replace('.tar.gz', '.zip');
    execSync(`powershell -Command "Compress-Archive -Path '${src}\\*' -DestinationPath '${zipDest}' -Force"`, {
      stdio: 'inherit',
    });
  } catch {
    // Fallback to tar on Unix-like systems
    try {
      execSync(`tar -czf "${dest}" .`, {
        cwd: src,
        stdio: 'inherit',
      });
    } catch (e) {
      console.warn('Archive creation skipped (platform not supported):', e.message);
    }
  }
}

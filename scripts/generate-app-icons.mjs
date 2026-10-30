#!/usr/bin/env node
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import { basename, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const reference = join(root, 'resources/brand/icon-reference.png');
const resources = join(root, 'resources/brand');
const desktopIcons = join(root, 'apps/desktop/icons');
const publicBrand = join(root, 'apps/frontend/public/brand');
const approvedSha256 = 'f67ff9721c85c360d4ba4f287403cc3d865e1da74b92c632d32ebd3e54d79aac';
const sourceBytes = readFileSync(reference);
if (createHash('sha256').update(sourceBytes).digest('hex') !== approvedSha256
  || !sourceBytes.subarray(0, 8).equals(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]))
  || sourceBytes.readUInt32BE(16) !== 1254 || sourceBytes.readUInt32BE(20) !== 1254
  || sourceBytes[24] !== 8 || sourceBytes[25] !== 6) {
  throw new Error('Icon source must match the approved 1254×1254, 8-bit RGBA PNG and SHA-256.');
}
const temporary = mkdtempSync(join(tmpdir(), 'image-studio-icons-'));

function sips(...args) {
  execFileSync('/usr/bin/sips', args, { stdio: 'ignore' });
}

function resize(source, size, target) {
  sips('-z', String(size), String(size), source, '--out', target);
}

function sha256(path) {
  return createHash('sha256').update(readFileSync(path)).digest('hex');
}

function buildIco(files, output) {
  const images = files.map(({ size, path }) => ({ size, bytes: readFileSync(path) }));
  const headerSize = 6;
  const entrySize = 16;
  let offset = headerSize + entrySize * images.length;
  const header = Buffer.alloc(headerSize);
  header.writeUInt16LE(0, 0);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(images.length, 4);
  const entries = images.map(({ size, bytes }) => {
    const entry = Buffer.alloc(entrySize);
    entry.writeUInt8(size === 256 ? 0 : size, 0);
    entry.writeUInt8(size === 256 ? 0 : size, 1);
    entry.writeUInt8(0, 2);
    entry.writeUInt8(0, 3);
    entry.writeUInt16LE(1, 4);
    entry.writeUInt16LE(32, 6);
    entry.writeUInt32LE(bytes.length, 8);
    entry.writeUInt32LE(offset, 12);
    offset += bytes.length;
    return entry;
  });
  writeFileSync(output, Buffer.concat([header, ...entries, ...images.map(image => image.bytes)]));
}

try {
  mkdirSync(resources, { recursive: true });
  mkdirSync(desktopIcons, { recursive: true });
  mkdirSync(publicBrand, { recursive: true });

  const master = join(resources, 'app-icon-1024.png');
  const uiCrop = join(temporary, 'ui-crop.png');
  const uiMaster = join(resources, 'app-icon-ui-512.png');
  const small = join(resources, 'app-icon-32.png');
  resize(reference, 1024, master);
  sips('-c', '1040', '1040', reference, '--out', uiCrop);
  resize(uiCrop, 512, uiMaster);
  resize(master, 32, small);

  copyFileSync(master, join(desktopIcons, 'icon.png'));
  copyFileSync(uiMaster, join(publicBrand, 'app-icon-ui.png'));
  copyFileSync(small, join(publicBrand, 'app-icon-32.png'));

  const iconset = join(temporary, 'AppIcon.iconset');
  mkdirSync(iconset, { recursive: true });
  const macSizes = [
    [16, 'icon_16x16.png'],
    [32, 'icon_16x16@2x.png'],
    [32, 'icon_32x32.png'],
    [64, 'icon_32x32@2x.png'],
    [128, 'icon_128x128.png'],
    [256, 'icon_128x128@2x.png'],
    [256, 'icon_256x256.png'],
    [512, 'icon_256x256@2x.png'],
    [512, 'icon_512x512.png'],
    [1024, 'icon_512x512@2x.png'],
  ];
  for (const [size, name] of macSizes) resize(master, size, join(iconset, name));
  const icns = join(temporary, 'AppIcon.icns');
  execFileSync('/usr/bin/iconutil', ['-c', 'icns', iconset, '-o', icns]);
  copyFileSync(icns, join(desktopIcons, 'icon.icns'));
  // iconutil may re-encode PNG data; record the extracted representation too.
  const extracted = join(temporary, 'Extracted.iconset');
  execFileSync('/usr/bin/iconutil', ['-c', 'iconset', icns, '-o', extracted]);

  const windowsSizes = [16, 24, 32, 48, 64, 128, 256];
  const windows = windowsSizes.map(size => {
    const path = join(temporary, `windows-${size}.png`);
    resize(master, size, path);
    return { size, path };
  });
  buildIco(windows, join(desktopIcons, 'icon.ico'));

  const outputs = [
    reference,
    master,
    uiMaster,
    small,
    join(desktopIcons, 'icon.png'),
    join(desktopIcons, 'icon.icns'),
    join(desktopIcons, 'icon.ico'),
    join(publicBrand, 'app-icon-ui.png'),
    join(publicBrand, 'app-icon-32.png'),
  ];
  const manifest = {
    source: basename(reference),
    sourceSha256: sha256(reference),
    macMaster: 'app-icon-1024.png',
    uiVariant: 'app-icon-ui-512.png',
    sourceFormat: { width: 1254, height: 1254, bitDepth: 8, colorType: 'RGBA' },
    uiCrop: { source: 'icon-reference.png', width: 1040, height: 1040, alignment: 'center', outputSize: 512 },
    generator: { node: process.version, macOS: execFileSync('/usr/bin/sw_vers', ['-productVersion'], { encoding: 'utf8' }).trim(), tools: ['sips', 'iconutil'] },
    macSizes: macSizes.map(([size, name]) => ({ size, name, sha256: sha256(join(iconset, name)), icnsExtractedSha256: sha256(join(extracted, name)) })),
    windowsIcoSizes: windowsSizes,
    windowsFrames: windows.map(({ size, path }) => ({ size, sha256: sha256(path) })),
    verifiedOn: {
      macOS: process.argv.includes('--mac-verified'),
      windows: false,
    },
    outputs: Object.fromEntries(outputs.map(path => [path.slice(root.length + 1), sha256(path)])),
  };
  writeFileSync(join(resources, 'manifest.json'), `${JSON.stringify(manifest, null, 2)}\n`);
} finally {
  rmSync(temporary, { recursive: true, force: true });
}

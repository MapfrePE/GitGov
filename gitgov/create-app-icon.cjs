const sharp = require('sharp');
const fs = require('fs');
const path = require('path');

const size = 1024;

const svg = `<svg width="${size}" height="${size}" xmlns="http://www.w3.org/2000/svg">
  <rect width="${size}" height="${size}" fill="#1d4ed8" rx="${size * 0.15}"/>
  <text x="50%" y="58%" dominant-baseline="middle" text-anchor="middle" 
        font-family="Arial, sans-serif" font-size="${size * 0.55}" font-weight="bold" fill="white">G</text>
</svg>`;

async function main() {
  await sharp(Buffer.from(svg))
    .png()
    .toFile(path.join(__dirname, 'app-icon.png'));
  console.log('app-icon.png created');
}

main().catch(console.error);

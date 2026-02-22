const sharp = require('sharp');
const pngToIco = require('png-to-ico');
const fs = require('fs');
const path = require('path');

const iconsDir = path.join(__dirname, 'src-tauri', 'icons');

async function main() {
  if (!fs.existsSync(iconsDir)) {
    fs.mkdirSync(iconsDir, { recursive: true });
  }

  const sizes = [16, 32, 48, 64, 128, 256];
  
  const pngBuffers = await Promise.all(
    sizes.map(async (size) => {
      const svg = `<svg width="${size}" height="${size}" xmlns="http://www.w3.org/2000/svg">
        <rect width="${size}" height="${size}" fill="#1d4ed8" rx="${size * 0.1}"/>
        <text x="50%" y="55%" dominant-baseline="middle" text-anchor="middle" 
              font-family="Arial, sans-serif" font-size="${size * 0.5}" font-weight="bold" fill="white">G</text>
      </svg>`;
      return sharp(Buffer.from(svg)).png().toBuffer();
    })
  );

  const icoBuffer = await pngToIco(pngBuffers);
  fs.writeFileSync(path.join(iconsDir, 'icon.ico'), icoBuffer);
  
  await sharp(pngBuffers[1]).toFile(path.join(iconsDir, '32x32.png'));
  await sharp(pngBuffers[4]).toFile(path.join(iconsDir, '128x128.png'));
  await sharp(pngBuffers[5]).toFile(path.join(iconsDir, '128x128@2x.png'));
  
  console.log('Icons created successfully');
}

main().catch(console.error);

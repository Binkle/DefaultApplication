#!/usr/bin/env python3
"""
Convert a source image to a visible RGBA app icon and generate PNG sizes
into src-tauri/icons. Centers the image on a 1024x1024 transparent canvas.

Usage:
  python3 scripts/convert_icon.py /absolute/or/relative/path/to/your/icon.png

Requires Pillow: pip install Pillow
"""
import sys
import os
from PIL import Image, ImageDraw
import argparse

SIZES = [32, 128, 256, 512, 1024]

def main():
  parser = argparse.ArgumentParser(description='Convert source image into app icons (RGBA), optionally with rounded corners.')
  parser.add_argument('src', help='Path to source image (PNG/JPG/SVG via Pillow loaders)')
  parser.add_argument('--rounded', action='store_true', help='Apply rounded corners mask to the output icons')
  parser.add_argument('--radius', type=int, default=0, help='Corner radius in pixels for 1024 icon (scaled for smaller sizes). If 0 with --rounded, uses 20% of size.')
  args = parser.parse_args()

  src_path = args.src
  if not os.path.exists(src_path):
    print(f"Source not found: {src_path}")
    sys.exit(1)

  out_dir = os.path.join('src-tauri', 'icons')
  os.makedirs(out_dir, exist_ok=True)

  # Load and convert to RGBA
  img = Image.open(src_path).convert('RGBA')

  # Center on 1024x1024 transparent canvas
  base_size = 1024
  canvas = Image.new('RGBA', (base_size, base_size), (0, 0, 0, 0))
  # Preserve aspect ratio
  img.thumbnail((base_size, base_size), Image.LANCZOS)
  x = (base_size - img.width) // 2
  y = (base_size - img.height) // 2
  canvas.paste(img, (x, y), img)

  def apply_rounded(im: Image.Image, r: int) -> Image.Image:
    if r <= 0:
      return im
    mask = Image.new('L', im.size, 0)
    draw = ImageDraw.Draw(mask)
    draw.rounded_rectangle([0, 0, im.size[0], im.size[1]], radius=r, fill=255)
    out = im.copy()
    out.putalpha(mask)
    return out

  if args.rounded:
    radius = args.radius if args.radius > 0 else int(base_size * 0.2)
    canvas = apply_rounded(canvas, radius)

  # Write 1024 and smaller sizes
  canvas.save(os.path.join(out_dir, 'icon.png'), 'PNG')
  for s in SIZES:
    resized = canvas.resize((s, s), Image.LANCZOS)
    if args.rounded:
      r = args.radius if args.radius > 0 else int(s * 0.2)
      resized = apply_rounded(resized, r)
    resized.save(os.path.join(out_dir, f'{s}x{s}.png'), 'PNG')

  print(f'Wrote icons to {out_dir}')

if __name__ == '__main__':
  main()

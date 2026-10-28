"""Rebuild the NSIS artwork from Qingjian's existing icon and Windows fonts."""
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'src-tauri' / 'installer'
OUT.mkdir(exist_ok=True)
FONT = Path('C:/Windows/Fonts/msyh.ttc')
if not FONT.exists():
    raise SystemExit('Microsoft YaHei is needed to reproduce the installer artwork')

def font(size):
    return ImageFont.truetype(str(FONT), size)

def icon(canvas, xy, size):
    mark = Image.open(ROOT / 'src-tauri/icons/128x128@2x.png').convert('RGBA')
    mark.thumbnail((size, size), Image.Resampling.LANCZOS)
    canvas.paste(mark, xy, mark)

header = Image.new('RGB', (150, 57), '#ffffff')
icon(header, (6, 10), 36)
draw = ImageDraw.Draw(header)
draw.text((50, 7), '轻笺', font=font(19), fill='#25272b')
draw.text((51, 33), '纯本地桌面便签', font=font(10), fill='#6b7280')
header.save(OUT / 'header.bmp')

side = Image.new('RGB', (164, 314), '#f4f5f7')
icon(side, (22, 25), 62)
draw = ImageDraw.Draw(side)
draw.text((25, 102), '轻笺', font=font(27), fill='#25272b')
draw.line((25, 153, 139, 153), fill='#d7dbe1', width=1)
draw.text((25, 175), '随手记录', font=font(16), fill='#3d424b')
draw.text((25, 201), '清晰推进', font=font(16), fill='#3d424b')
draw.text((25, 269), '简单 · 轻量 · 本地', font=font(10), fill='#6b7280')
side.save(OUT / 'sidebar.bmp')
print('Generated NSIS header 150x57 and sidebar 164x314')

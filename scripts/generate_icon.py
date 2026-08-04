from pathlib import Path
from PIL import Image, ImageDraw


ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "src-tauri" / "icons"
OUT.mkdir(parents=True, exist_ok=True)


def draw_icon(size: int) -> Image.Image:
    scale = 4
    canvas = Image.new("RGBA", (size * scale, size * scale), (0, 0, 0, 0))
    draw = ImageDraw.Draw(canvas)

    def box(values):
        return tuple(round(value * size * scale / 512) for value in values)

    draw.rounded_rectangle(box((44, 44, 468, 468)), radius=round(108 * size * scale / 512), fill="#302D27")
    draw.polygon(
        [box((145, 112))[0:2], box((323, 112))[0:2], box((399, 188))[0:2],
         box((399, 400))[0:2], box((145, 400))[0:2]],
        fill="#F8D46F",
    )
    draw.polygon(
        [box((323, 112))[0:2], box((323, 188))[0:2], box((399, 188))[0:2]],
        fill="#FFE9A4",
    )
    line_width = max(2, round(19 * size * scale / 512))
    for x2, y in [(348, 235), (322, 282), (339, 329)]:
        draw.line(
            [box((196, y))[0:2], box((x2, y))[0:2]],
            fill="#6F5924",
            width=line_width,
        )
    return canvas.resize((size, size), Image.Resampling.LANCZOS)


icon_32 = draw_icon(32)
icon_128 = draw_icon(128)
icon_256 = draw_icon(256)
icon_32.save(OUT / "32x32.png")
icon_128.save(OUT / "128x128.png")
icon_256.save(OUT / "128x128@2x.png")
icon_256.save(
    OUT / "icon.ico",
    format="ICO",
    sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
)


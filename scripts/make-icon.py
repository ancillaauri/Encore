"""Draws Encore's icon (a glowing E over a waveform) and writes every size the app needs.
Run from the repo root:  python scripts/make-icon.py   (needs Pillow)"""
import math, os
from PIL import Image, ImageDraw, ImageFilter, ImageChops

S = 1024; C = S / 2
BG, EDGE = (12, 11, 16), (40, 39, 46)
BLUE, LBLUE, CORE, WAVE = (70, 88, 255), (130, 150, 255), (236, 240, 255), (62, 76, 200)

# Waveform shape: tallest in the middle, curving down and fading outwards, then rising
# back slightly near the edges.
WAVE_HEIGHT = 700 * 0.75   # height of the centre bar
WAVE_DROP = 340 * 1.10     # how far the bars fall by the dip
DIP_AT = 0.72              # where the dip bottoms out (0 centre, 1 edge)
RETURN_HEIGHT = 0.18       # how much height comes back at the edge
RETURN_FADE = 0.10         # how much brightness comes back at the edge
BARS, BAR_WIDTH, BAR_STEP, STRENGTH = 17, 12, 46, 0.65

def solid(mask, colour, alpha=1.0):
    out = Image.new("RGBA", (S, S), colour + (0,))
    out.putalpha(mask.point(lambda v: int(min(255, v * alpha))))
    return out

def profile(t):
    h0 = WAVE_HEIGHT - WAVE_DROP * DIP_AT ** 1.25
    a0 = math.exp(-(DIP_AT * 2.0) ** 2)
    if t <= DIP_AT:
        return WAVE_HEIGHT - WAVE_DROP * t ** 1.25, math.exp(-(t * 2.0) ** 2)
    s = math.sin((t - DIP_AT) / (1 - DIP_AT) * math.pi / 2)
    return h0 + (WAVE_HEIGHT - h0) * RETURN_HEIGHT * s, a0 + RETURN_FADE * s

def waveform():
    m = Image.new("L", (S, S), 0); d = ImageDraw.Draw(m)
    x0 = C - (BARS - 1) * BAR_STEP / 2
    for i in range(BARS):
        t = abs((i - (BARS - 1) / 2) / ((BARS - 1) / 2))
        h, a = profile(t); x = x0 + i * BAR_STEP
        d.rounded_rectangle([x - BAR_WIDTH / 2, C - h / 2, x + BAR_WIDTH / 2, C + h / 2],
                            radius=BAR_WIDTH / 2, fill=int(255 * STRENGTH * a))
    return m

def letter_e(r=30, stem=92, gap=28, arm=280, mid=200, h=470, bar=92):
    m = Image.new("L", (S, S), 0); d = ImageDraw.Draw(m)
    x0 = C - (stem + gap + arm) / 2; top = C - h / 2
    d.rounded_rectangle([x0, top, x0 + stem, top + h], radius=r, fill=255)
    for y, length in [(top, arm), (top + h / 2 - bar / 2, mid), (top + h - bar, arm)]:
        d.rounded_rectangle([x0 + stem + gap, y, x0 + stem + gap + length, y + bar], radius=r, fill=255)
    return m

def neon(img, mask):
    img = Image.alpha_composite(img, solid(mask.filter(ImageFilter.GaussianBlur(55)), BLUE, 0.85))
    img = Image.alpha_composite(img, solid(mask.filter(ImageFilter.GaussianBlur(18)), LBLUE, 0.7))
    img = Image.alpha_composite(img, solid(mask, LBLUE))
    inner = mask.filter(ImageFilter.MinFilter(11)).filter(ImageFilter.GaussianBlur(5))
    return Image.alpha_composite(img, solid(inner, CORE))

def icon():
    tile = Image.new("L", (S, S), 0)
    ImageDraw.Draw(tile).rounded_rectangle([40, 40, S - 40, S - 40], radius=220, fill=255)
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    img.paste(Image.new("RGBA", (S, S), BG + (255,)), (0, 0), tile)
    img = Image.alpha_composite(img, solid(waveform(), WAVE))
    img = neon(img, letter_e())
    out = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    out.paste(img, (0, 0), ImageChops.multiply(img.getchannel("A"), tile))
    ImageDraw.Draw(out).rounded_rectangle([40, 40, S - 40, S - 40], radius=220, outline=EDGE + (255,), width=8)
    return out

if __name__ == "__main__":
    img = icon()
    img.save("src-tauri/icons/icon.png")
    for size, name in [(32, "32x32.png"), (128, "128x128.png"), (256, "128x128@2x.png")]:
        img.resize((size, size), Image.LANCZOS).save(f"src-tauri/icons/{name}")
    img.save("src-tauri/icons/icon.ico", sizes=[(n, n) for n in (16, 24, 32, 48, 64, 128, 256)])
    os.makedirs("ui/img", exist_ok=True)
    img.resize((64, 64), Image.LANCZOS).save("ui/img/icon.png")
    print("icons written")

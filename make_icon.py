from PIL import Image, ImageDraw, ImageFilter, ImageFont
import math
S = 1024
def grad(c0, c1):
    g = Image.new('RGBA', (S, S)); d = ImageDraw.Draw(g)
    for y in range(S):
        t = y / S; d.line([(0, y), (S, y)], fill=tuple(int(a + (b - a) * t) for a, b in zip(c0, c1)) + (255,))
    return g
def shield(scale, off=0):
    w = S * scale; cx = S / 2; top = S * (0.5 - scale * 0.47) + off; pts = []
    for t in range(101):
        x = -1 + 2 * t / 100; pts.append((cx + x * w * 0.42, top + (abs(x) ** 6) * w * 0.04))
    for t in range(101):
        a = t / 100 * math.pi; pts.append((cx + math.cos(a) * w * 0.42, top + w * 0.42 + math.sin(a) ** 0.9 * w * 0.52))
    return pts
im = Image.new('RGBA', (S, S), (0, 0, 0, 0))
m = Image.new('L', (S, S), 0); ImageDraw.Draw(m).polygon(shield(0.98), fill=255)
im.paste(grad((255, 230, 150), (195, 120, 40)), (0, 0), m)
m2 = Image.new('L', (S, S), 0); ImageDraw.Draw(m2).polygon(shield(0.86, 8), fill=255)
im.paste(grad((48, 36, 22), (23, 16, 8)), (0, 0), m2)
f = ImageFont.truetype(r'C:\Windows\Fonts\georgiab.ttf', int(S * 0.42))
tm = Image.new('L', (S, S), 0); td = ImageDraw.Draw(tm)
bb = td.textbbox((0, 0), 'II', font=f)
td.text(((S - (bb[2] - bb[0])) / 2 - bb[0], S * 0.47 - (bb[3] - bb[1]) / 2 - bb[1]), 'II', font=f, fill=255)
sh = Image.new('RGBA', (S, S), (0, 0, 0, 0)); sh.paste((0, 0, 0, 200), (0, 0), tm.filter(ImageFilter.GaussianBlur(10)))
im = Image.alpha_composite(im, sh.transform((S, S), Image.AFFINE, (1, 0, -6, 0, 1, -10)))
im.paste(grad((255, 240, 190), (215, 120, 40)), (0, 0), tm)
im.resize((256, 256), Image.LANCZOS).save('icon.ico', sizes=[(256, 256), (64, 64), (48, 48), (32, 32), (24, 24), (16, 16)])
print('ok')

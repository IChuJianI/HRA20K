#!/usr/bin/env python3
"""Compose HRA20K PC promo images (1920x1080) in the DiskInfo/WFun promo language.

Screens are real captures of HRA20K running on HarmonyOS 2in1, cropped to the
laptop viewport and nothing else. Drop the captures into promo_src/ as
screen_menu.png, screen_lobby.png and screen_battle.png, then run this script.
"""
import math
import os

from PIL import Image, ImageDraw, ImageFilter, ImageFont

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
SRC = os.path.join(ROOT, 'tools/promo/promo_src')
OUT = os.path.join(ROOT, 'store/promo')
FONT_PATH = '/system/fonts/HarmonyOS_Sans_SC.ttf'
SS = 2

HEAD = (244, 247, 252)
HEAD_ACCENT = (255, 184, 84)
DESC = (166, 176, 190)
CHIP_BG = (14, 13, 18)
CHIP_LINE = (52, 40, 30)
CHIP_TEXT = (224, 218, 208)
CHECK_RING = (150, 108, 56)
CHECK_TEXT = (204, 208, 216)
EYEBROW = (240, 176, 120)
NOTE = (150, 140, 130)
SCREEN_W, SCREEN_H = 1160, 740

_fonts = {}


def font(size, weight='Regular'):
    key = (max(1, int(round(size))), weight)
    if key not in _fonts:
        face = ImageFont.truetype(FONT_PATH, key[0])
        try:
            face.set_variation_by_name(weight)
        except Exception:
            pass
        _fonts[key] = face
    return _fonts[key]


class P:
    def __init__(self, img, ss=SS):
        self.img = img
        self.ss = ss
        self.d = ImageDraw.Draw(img)

    def px(self, v):
        return v * self.ss

    def text(self, xy, s, size, fill, weight='Regular', anchor='ls'):
        self.d.text((self.px(xy[0]), self.px(xy[1])), s, font=font(size * self.ss, weight),
                    fill=fill, anchor=anchor)

    def rect(self, box, fill=None, outline=None, width=1, radius=0):
        coords = [self.px(box[0]), self.px(box[1]), self.px(box[2]), self.px(box[3])]
        w = max(1, int(round(self.px(width))))
        if radius > 0:
            self.d.rounded_rectangle(coords, radius=self.px(radius), fill=fill,
                                     outline=outline, width=w)
        else:
            self.d.rectangle(coords, fill=fill, outline=outline, width=w)

    def line(self, xy, fill, width=1):
        self.d.line([(self.px(xy[0]), self.px(xy[1])), (self.px(xy[2]), self.px(xy[3]))],
                    fill=fill, width=max(1, int(round(self.px(width)))))

    def poly(self, points, fill, width=1):
        coords = [(self.px(x), self.px(y)) for (x, y) in points]
        self.d.line(coords, fill=fill, width=max(1, int(round(self.px(width)))), joint='curve')

    def ellipse(self, box, fill=None, outline=None, width=1):
        self.d.ellipse([self.px(box[0]), self.px(box[1]), self.px(box[2]), self.px(box[3])],
                       fill=fill, outline=outline, width=max(1, int(round(self.px(width)))))

    def text_width(self, s, size, weight='Regular'):
        return font(size * self.ss, weight).getlength(s) / self.ss


def icon_check(p, cx, cy, s, color, width=2.2):
    p.poly([(cx - s * 0.42, cy + s * 0.02), (cx - s * 0.1, cy + s * 0.34),
            (cx + s * 0.46, cy - s * 0.34)], color, width)


def icon_check_circle(p, cx, cy, s, ring, tick, width=2):
    p.ellipse([cx - s / 2, cy - s / 2, cx + s / 2, cy + s / 2], outline=ring, width=width)
    icon_check(p, cx, cy, s * 0.62, tick, width * 1.1)


def icon_image(size):
    key = int(round(size))
    cache = icon_image.cache
    if key not in cache:
        base = os.path.join(ROOT, 'AppScope/resources/base/media/')
        bg = Image.open(base + 'background.png').convert('RGBA')
        fg = Image.open(base + 'foreground.png').convert('RGBA').resize(bg.size, Image.LANCZOS)
        bg.alpha_composite(fg)
        cache[key] = bg.resize((key, key), Image.LANCZOS)
    return cache[key]


icon_image.cache = {}


def rounded_icon(size, radius_ratio=0.225):
    key = ('r', int(round(size)))
    cache = icon_image.cache
    if key not in cache:
        icon = icon_image(size)
        mask = Image.new('L', (size * 2, size * 2), 0)
        ImageDraw.Draw(mask).rounded_rectangle([0, 0, size * 2 - 1, size * 2 - 1],
                                               radius=int(size * 2 * radius_ratio), fill=255)
        mask = mask.resize((size, size), Image.LANCZOS)
        card = Image.new('RGBA', (size, size), (0, 0, 0, 0))
        card.paste(icon, (0, 0), mask)
        cache[key] = card
    return cache[key]


def screen_from(path, crop=None):
    image = Image.open(path).convert('RGB')
    if crop:
        image = image.crop(crop)
    w, h = image.size
    scale = min(SCREEN_W / float(w), SCREEN_H / float(h))
    size = (max(1, int(round(w * scale))), max(1, int(round(h * scale))))
    image = image.resize(size, Image.LANCZOS)
    screen = Image.new('RGB', (SCREEN_W, SCREEN_H), (0, 0, 0))
    screen.paste(image, ((SCREEN_W - size[0]) // 2, (SCREEN_H - size[1]) // 2))
    return screen


def make_glow_background(width, height, core, wide):
    small_w, small_h = 480, 320
    base = Image.new('RGB', (small_w, small_h), (5, 7, 12))
    px = base.load()
    cx, cy = core
    wx, wy = wide
    for y in range(small_h):
        for x in range(small_w):
            nx = x / float(small_w)
            ny = y / float(small_h)
            d1 = math.hypot(nx - cx, (ny - cy) * 0.85)
            d2 = math.hypot(nx - wx, (ny - wy) * 0.7)
            g1 = max(0.0, 1.0 - d1 / 0.62) ** 2
            g2 = max(0.0, 1.0 - d2 / 1.05) ** 2
            r = 5 + 46 * g1 + 8 * g2
            g = 7 + 15 * g1 + 22 * g2
            b = 12 + 18 * g1 + 58 * g2
            px[x, y] = (int(min(255, r)), int(min(255, g)), int(min(255, b)))
    img = base.resize((width, height), Image.LANCZOS)
    noise = Image.effect_noise((width, height), 7).convert('L')
    noise_rgba = Image.new('RGBA', (width, height), (0, 0, 0, 0))
    noise_rgba.putalpha(noise.point(lambda v: int(v * 0.035)))
    img = img.convert('RGBA')
    img.alpha_composite(noise_rgba)
    return img


def cast_shadow(canvas, box, blur):
    shadow = Image.new('RGBA', canvas.size, (0, 0, 0, 0))
    ImageDraw.Draw(shadow).ellipse([box[0], box[1], box[2], box[3]], fill=(0, 0, 0, 150))
    shadow = shadow.filter(ImageFilter.GaussianBlur(blur))
    canvas.alpha_composite(shadow)


def laptop_frame(app, screen_w=SCREEN_W, screen_h=SCREEN_H):
    screen = app.resize((screen_w, screen_h), Image.LANCZOS)
    bezel = 12
    fw, fh = screen_w + bezel * 2, screen_h + bezel * 2
    frame = Image.new('RGBA', (fw, fh + 34), (0, 0, 0, 0))
    d = ImageDraw.Draw(frame)
    d.rounded_rectangle([0, 0, fw - 1, fh - 1], radius=20, fill=(20, 22, 25, 255))
    d.rounded_rectangle([1, 1, fw - 2, fh - 2], radius=19, outline=(48, 52, 58, 255), width=2)
    mask = Image.new('L', (screen_w * 2, screen_h * 2), 0)
    ImageDraw.Draw(mask).rounded_rectangle([0, 0, screen_w * 2 - 1, screen_h * 2 - 1],
                                           radius=10, fill=255)
    mask = mask.resize((screen_w, screen_h), Image.LANCZOS)
    frame.paste(screen, (bezel, bezel), mask)
    d = ImageDraw.Draw(frame)
    d.ellipse([fw / 2.0 - 2.5, bezel / 2.0 - 2.5, fw / 2.0 + 2.5, bezel / 2.0 + 2.5],
              fill=(10, 12, 14, 255))
    base_w = fw + 136
    base_x = (fw - base_w) / 2.0
    d.rounded_rectangle([base_x, fh - 2, base_x + base_w, fh + 30], radius=12,
                        fill=(42, 46, 50, 255))
    d.rectangle([base_x + 12, fh - 2, base_x + base_w - 12, fh + 3], fill=(64, 69, 74, 255))
    d.rounded_rectangle([fw / 2.0 - 110, fh + 1, fw / 2.0 + 110, fh + 11], radius=5,
                        fill=(15, 17, 19, 255))
    d.line([base_x + 3, fh + 30, base_x + base_w - 3, fh + 30], fill=(22, 24, 26, 255), width=2)
    return frame


def draw_brand(canvas, p, left, top):
    canvas.alpha_composite(rounded_icon(64), (left, top))
    name_x = left + 64 + 18
    p.text((name_x, top + 45), 'HRA20K', 40, HEAD, 'SemiBold')


def draw_badge(p, right, top, label='鸿蒙原生'):
    tw = p.text_width(label, 20)
    w = 34 + tw + 40
    x = right - w
    p.rect([x, top, right, top + 40], radius=20, fill=(16, 11, 12), outline=(60, 40, 32), width=1)
    p.ellipse([x + 18, top + 15, x + 28, top + 25], fill=HEAD_ACCENT)
    p.text((x + 36, top + 21), label, 20, (206, 190, 174))


def draw_copy(p, spec):
    eyebrow_y, line1_y, line2_y = 158, 265, 357
    desc_y, chips_y, check_y, footer_y = 408, 482, 580, 1025

    p.line([110, eyebrow_y - 10, 150, eyebrow_y - 10], HEAD_ACCENT, 3)
    p.text((160, eyebrow_y), spec['eyebrow'], 22, EYEBROW, 'Medium')

    p.text((108, line1_y), spec['line1'], 92, HEAD, 'Bold')
    p.text((108, line2_y), spec['line2'], 92, HEAD_ACCENT, 'Bold')

    dy = desc_y
    for line in spec['desc']:
        p.text((110, dy), line, 30, DESC)
        dy += 42

    cx = 110
    for chip in spec['chips']:
        tw = p.text_width(chip, 21)
        w = 18 + 10 + 12 + tw + 18
        p.rect([cx, chips_y, cx + w, chips_y + 44], radius=22, fill=CHIP_BG,
               outline=CHIP_LINE, width=1)
        p.ellipse([cx + 18, chips_y + 17, cx + 28, chips_y + 27], fill=HEAD_ACCENT)
        p.text((cx + 40, chips_y + 23), chip, 21, CHIP_TEXT)
        cx += w + 12

    for index, row in enumerate(spec['checks']):
        cy = check_y + index * 52 + 13
        icon_check_circle(p, 123, cy, 26, CHECK_RING, HEAD_ACCENT, 2)
        p.text((152, cy + 9), row, 26, CHECK_TEXT)

    p.text((110, footer_y), spec['footer'], 24, NOTE)


def compose_pc(spec):
    W, H = 1920, 1080
    canvas = make_glow_background(W, H, (0.72, 0.44), (0.24, 0.52))
    cast_shadow(canvas, (700, 930, 1880, 1045), 45)
    canvas.alpha_composite(laptop_frame(screen_from(os.path.join(SRC, spec['screen']),
                                                    spec.get('crop'))), (690, 195))
    p = P(canvas, 1)
    draw_brand(canvas, p, 110, 62)
    draw_badge(p, 1810, 74)
    draw_copy(p, spec)
    return canvas.convert('RGB')


PC = [
    {'name': 'pc_01_mainmenu', 'screen': 'screen_menu.png',
     'crop': (0, 0, 3120, 1995),
     'eyebrow': '电脑端 · 鸿蒙原生',
     'line1': '经典红警', 'line2': '战火重燃',
     'desc': ['原版主菜单、音乐与中文按钮', '需自备正版《尤里的复仇》文件'],
     'chips': ['全屏沉浸', '中文界面', '键鼠操作'],
     'checks': ['原版主菜单与音轨资源', '中文菜单与选项', '2in1 全屏键鼠适配'],
     'footer': '鸿蒙电脑 · 需自备正版游戏文件'},
    {'name': 'pc_02_lobby', 'screen': 'screen_lobby.png',
     'eyebrow': '电脑端 · 联机大厅',
     'line1': '联机大厅', 'line2': '随时开战',
     'desc': ['创建对局、快速配对与大厅聊天', '和在线玩家一起重返战场'],
     'chips': ['快速配对', '大厅聊天', '在线玩家'],
     'checks': ['创建或加入自定义对局', '大厅聊天与在线玩家列表', '键鼠全屏操作'],
     'footer': '鸿蒙电脑 · 需自备正版游戏文件'},
    {'name': 'pc_03_battle', 'screen': 'screen_battle.png',
     'eyebrow': '电脑端 · 即时战略',
     'line1': '经典对战', 'line2': '原味战场',
     'desc': ['基地建造、单位指挥与资源采集', '完整还原尤里的复仇玩法'],
     'chips': ['实时对战', '原版单位', '全屏指挥'],
     'checks': ['原版单位、建筑与科技树', '中文界面与提示', '宽屏全屏战场'],
     'footer': '鸿蒙电脑 · 需自备正版游戏文件'},
]


def main():
    os.makedirs(OUT, exist_ok=True)
    missing = [spec['screen'] for spec in PC if not os.path.exists(os.path.join(SRC, spec['screen']))]
    if missing:
        raise SystemExit('缺少截图: ' + ', '.join(missing))
    for spec in PC:
        image = compose_pc(spec)
        target = os.path.join(OUT, spec['name'] + '.png')
        image.save(target)
        print(target, image.size)


if __name__ == '__main__':
    main()
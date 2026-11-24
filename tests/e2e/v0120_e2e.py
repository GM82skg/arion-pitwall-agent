"""Driver app v0.1.20 / dashboard v0.1.15: tab icon = white horse head (built in, 64 px PNG);
«Pit Wall» with the version under it, together no taller than the ARION logo, on desktop and phone."""
import asyncio, subprocess, time, sys, base64, io
from playwright.async_api import async_playwright
BIN = sys.argv[1]; HERE = __file__.rsplit('/', 1)[0]; ok = fail = 0
def chk(n, c, info=''):
    global ok, fail
    if c: ok += 1; print('PASS', n)
    else: fail += 1; print('FAIL', n, info)
import portwait; portwait.wait_free()
rest = subprocess.Popen([sys.executable, f'{HERE}/mock_lmu_rest.py', '6397'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
ag = subprocess.Popen([BIN, '--key', 'arion_tk_' + 'ab' * 32, '--fake', '--fake-speed', '10', '--url', 'http://127.0.0.1:9', '--apikey', 'x', '--seconds', '60'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
time.sleep(1.5)
BOX = """(()=>{const w=document.querySelector('.brand .wm').getBoundingClientRect(),t=document.querySelector('.brand .bt').getBoundingClientRect(),
  b=document.querySelector('.brand b').getBoundingClientRect(),v=document.querySelector('#bver').getBoundingClientRect();
  return {wt:w.top,wb:w.bottom,wh:w.height,tt:t.top,tb:t.bottom,th:t.height,bb:b.bottom,vt:v.top,vb:v.bottom,bl:b.left,vl:v.left,
    txt:document.querySelector('#bver').textContent,pw:document.querySelector('.brand b').textContent,
    vcol:getComputedStyle(document.querySelector('#bver')).color}})()"""
def fits(r):
    e = 0.5
    return r['th'] <= r['wh'] + e and r['tt'] >= r['wt'] - e and r['tb'] <= r['wb'] + e and r['bb'] <= r['vt'] + e
async def main():
    async with async_playwright() as p:
        br = await p.chromium.launch()
        for name, vp, mob in [('desktop', {'width': 1440, 'height': 900}, False), ('phone', {'width': 390, 'height': 844}, True)]:
            ctx = await br.new_context(viewport=vp, is_mobile=mob, has_touch=mob, device_scale_factor=2 if mob else 1)
            pg = await ctx.new_page(); errs = []; pg.on('pageerror', lambda e: errs.append(str(e)[:200]))
            await pg.route(lambda u: not u.startswith('http://127.0.0.1') and not u.startswith('data:'), lambda r: r.abort())
            await pg.goto('http://127.0.0.1:6420/'); await asyncio.sleep(6)
            E = pg.evaluate
            chk(f'{name}: dashboard 0.1.22', await E('VERSION') == '0.1.22')
            if name == 'desktop':
                href = await E('(document.querySelector("link[rel=icon]")||{}).href||""')
                ok_png = False; info = href[:60]
                if href.startswith('data:image/png;base64,'):
                    from PIL import Image
                    im = Image.open(io.BytesIO(base64.b64decode(href.split(',', 1)[1])))
                    raw = im.convert('RGBA').tobytes(); allpx = [tuple(raw[i:i + 4]) for i in range(0, len(raw), 4)]
                    px = [q for q in allpx if q[3] > 40]
                    white = sum(1 for q in px if min(q[:3]) > 200)
                    clear = sum(1 for q in allpx if q[3] == 0)
                    ok_png = im.size == (64, 64) and px and white / len(px) > 0.9 and clear > 64 * 64 * 0.3
                    info = (im.size, len(px), white, clear)
                chk('tab icon: built-in 64 px PNG, white on transparent', ok_png, info)
            r = await E(BOX)
            chk(f'{name}: title reads «Pit Wall», version «v0.1.22» under it', r['pw'] == 'Pit Wall' and r['txt'] == 'v0.1.22' and r['vt'] >= r['bb'] - 0.5 and abs(r['bl'] - r['vl']) < 1, r)
            chk(f'{name}: «Pit Wall» + version no taller than the ARION logo, inside its top and bottom', fits(r), r)
            chk(f'{name}: version in the grey of secondary text', r['vcol'] == 'rgb(140, 143, 152)', r['vcol'])
            print(f'    {name}: logo {r["wh"]:.1f} px, title block {r["th"]:.1f} px')
            planted = dict(r, th=r['wh'] + 6, tb=r['wb'] + 6)
            chk(f'{name}: the height check catches a planted taller block', not fits(planted))
            chk(f'{name}: no page errors', errs == [], errs)
            await pg.screenshot(path=f'/tmp/v0120_{name}.png', clip={'x': 0, 'y': 0, 'width': vp['width'], 'height': 110})
            await ctx.close()
        await br.close()
try: asyncio.run(main())
finally:
    ag.terminate(); rest.terminate()
print(f'\n{ok} passed, {fail} failed')

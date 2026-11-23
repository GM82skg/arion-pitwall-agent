"""Phone layout: five tabs, sideways full screen, no sideways scrolling; tablets keep the desktop layout."""
import asyncio, subprocess, time, sys
from playwright.async_api import async_playwright
BIN = sys.argv[1]; HERE = __file__.rsplit('/', 1)[0]; ok = fail = 0
def chk(n, c, info=''):
    global ok, fail
    if c: ok += 1; print('PASS', n)
    else: fail += 1; print('FAIL', n, info)
import portwait; portwait.wait_free()   # the previous test's app may still hold 6420
rest = subprocess.Popen([sys.executable, f'{HERE}/mock_lmu_rest.py', '6397'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
ag = subprocess.Popen([BIN, '--key', 'arion_tk_' + 'ab' * 32, '--fake', '--fake-speed', '10', '--url', 'http://127.0.0.1:9', '--apikey', 'x', '--seconds', '70'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
time.sleep(1.5)
async def main():
    async with async_playwright() as p:
        br = await p.chromium.launch(); errs = []
        ctx = await br.new_context(viewport={'width': 390, 'height': 844}, is_mobile=True, has_touch=True, device_scale_factor=2)
        pg = await ctx.new_page(); pg.on('pageerror', lambda e: errs.append(str(e)[:200]))
        await pg.route(lambda u: not u.startswith('http://127.0.0.1'), lambda r: r.abort())
        await pg.goto('http://127.0.0.1:6420/'); await asyncio.sleep(16)
        E = pg.evaluate
        chk('phone layout on, four tabs at the bottom', await E('PHONE') and await E('document.querySelectorAll(".pnav button").length') == 4)
        # v0.1.14: tab icons are drawn line icons, all the same size and weight, centred over their label; selected = yellow
        NAV = """(()=>{const bs=[...document.querySelectorAll('.pnav button')];const y=getComputedStyle(document.documentElement).getPropertyValue('--y').trim();
          const t=document.createElement('i');t.style.color=y;document.body.appendChild(t);const yc=getComputedStyle(t).color;t.remove();
          return {bar:Math.round(document.querySelector('.pnav').getBoundingClientRect().height),yc,items:bs.map(b=>{const i=b.querySelector('b'),sv=i&&i.querySelector('svg'),r=sv?sv.getBoundingClientRect():null,br=b.getBoundingClientRect(),lb=[...b.childNodes].filter(n=>n.nodeType===3).map(n=>n.textContent).join('');
            return {t:b.dataset.t,svg:!!sv,glyph:i?i.textContent.trim():'',w:r?Math.round(r.width):0,h:r?Math.round(r.height):0,sw:sv?sv.getAttribute('stroke-width'):null,
              cx:r?Math.abs((r.left+r.right)/2-(br.left+br.right)/2):99,col:getComputedStyle(b).color,ic:sv?getComputedStyle(sv).color:'',on:b.classList.contains('on'),label:lb}})}})()"""
        def navok(n):
            its = n['items']
            return (len(its) == 4 and all(i['svg'] and i['glyph'] == '' and i['w'] == 22 and i['h'] == 22 and i['sw'] == '1.8' and i['cx'] <= 1 for i in its))
        n = await E(NAV)
        chk('tab bar: four drawn icons, 22 px, same line weight, centred, no text characters', navok(n), n)
        chk('tab bar: labels Car · Map · Timing · Strategy', [i['label'] for i in n['items']] == ['Car', 'Map', 'Timing', 'Strategy'], [i['label'] for i in n['items']])
        chk('tab bar: only the selected tab is yellow (icon and label)', [i['col'] == n['yc'] and i['ic'] == n['yc'] for i in n['items']] == [i['on'] for i in n['items']] == [True, False, False, False], n['items'])
        chk('tab bar: no taller than before (≤ 60 px)', n['bar'] <= 60, n['bar'])
        planted = dict(n); planted['items'] = [dict(n['items'][0], svg=False, glyph='◉')] + n['items'][1:]
        chk('tab bar check catches a planted text icon', not navok(planted))
        print('    tab bar height', n['bar'])
        t = await E('document.querySelector("#pcar").innerText')
        chk('Car tab: delta row, lap times, ahead/behind, energy, tyres, damage, moments', all(x in t for x in ['proj', 'pot', 'Last', 'Best', 'Laps', 'Behind', 'Energy', 'FL', 'Damage', 'Aero 14%']), t[:300])
        sw = lambda: E('[document.documentElement.scrollWidth, innerWidth]')
        r = await sw(); chk('Car tab: no sideways scrolling', r[0] <= r[1], r)
        await pg.screenshot(path='/tmp/ph_car.png')
        for tab, test in [('map', 'S.mode==="follow"&&!!document.querySelector("#map")&&document.querySelector("#map").width>300&&!!document.querySelector("#inp")&&S.hist.length>20'),
                          ('timing', 'document.querySelectorAll("#laps tr").length>=3'),
                          ('strat', '!!document.querySelector("#battle tr")&&document.querySelector("#pitw").innerText.includes("Rejoin")&&document.querySelectorAll("#wx .slot").length===5')]:
            await pg.click(f'.pnav button[data-t="{tab}"]'); await asyncio.sleep(1.2)
            chk(f'{tab} tab shows its content', await E(test), await E('[S.laps.length, document.querySelectorAll("#laps tr").length, S.gridTab, document.querySelector("#laps")?.innerText.slice(0,80)]'))
            r = await sw(); chk(f'{tab} tab: no sideways scrolling', r[0] <= r[1], r)
            n = await E(NAV); chk(f'{tab} tab: yellow moves to {tab}', [i['col'] == n['yc'] for i in n['items']] == [i['t'] == tab for i in n['items']], [(i['t'], i['col']) for i in n['items']])
            await pg.screenshot(path=f'/tmp/ph_{tab}.png')
        await pg.click('.pnav button[data-t="timing"]'); await asyncio.sleep(0.6)
        fits = await E('(()=>{const t=document.querySelector("#laps table"),p=document.querySelector("#laps");return t?Math.round(t.getBoundingClientRect().right)<=Math.round(p.getBoundingClientRect().right)+1:false})()')
        chk('timing table fits the screen (no clipped column)', fits)
        await pg.click('.pnav button[data-t="strat"]'); await asyncio.sleep(0.6)
        fits = await E('(()=>{const t=document.querySelector("#battle table"),p=document.querySelector("#battle");return t?Math.round(t.getBoundingClientRect().right)<=Math.round(p.getBoundingClientRect().right):false})()')
        chk('battle table fits the screen', fits)
        # sideways on Map: map and inputs fill the screen side by side
        await pg.click('.pnav button[data-t="map"]'); await pg.set_viewport_size({'width': 844, 'height': 390}); await asyncio.sleep(1.2)
        land = await E('[document.body.classList.contains("land"), getComputedStyle(document.querySelector(".top")).display, getComputedStyle(document.querySelector(".pnav")).display, Math.round(document.querySelector("#inp").getBoundingClientRect().height)]')
        chk('sideways on Map: map and inputs full screen', land[0] and land[1] == 'none' and land[2] == 'none' and land[3] > 250, land)
        await pg.screenshot(path='/tmp/ph_inputs_land.png')
        await pg.set_viewport_size({'width': 390, 'height': 844}); await asyncio.sleep(0.8)
        chk('back upright: tab bar returns', await E('getComputedStyle(document.querySelector(".pnav")).display') != 'none')
        chk('no page errors', not errs, errs)
        # a tablet keeps the desktop panels
        t2 = await (await br.new_context(viewport={'width': 1024, 'height': 768}, has_touch=True)).new_page()
        await t2.route(lambda u: not u.startswith('http://127.0.0.1'), lambda r: r.abort())
        await t2.goto('http://127.0.0.1:6420/'); await asyncio.sleep(2)
        chk('tablet: desktop layout with panels', not await t2.evaluate('PHONE') and await t2.evaluate('document.querySelectorAll(".dock").length') == 10)
        await br.close()
asyncio.run(main())
ag.terminate(); rest.terminate()
print(f'\n{ok} passed, {fail} failed')

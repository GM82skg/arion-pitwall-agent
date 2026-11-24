"""Driver app v0.1.22 / dashboard v0.1.17: the layout engine — drop on a panel's half to split it, on its centre to add a tab,
on a gutter for a new column (or row) there; old layouts converted; everything remembered after a reload."""
import asyncio, subprocess, time, sys, json
from playwright.async_api import async_playwright
BIN = sys.argv[1]; HERE = __file__.rsplit('/', 1)[0]; ok = fail = 0
def chk(n, c, info=''):
    global ok, fail
    if c: ok += 1; print('PASS', n)
    else: fail += 1; print('FAIL', n, json.dumps(info)[:700] if not isinstance(info, str) else info[:700])
import portwait; portwait.wait_free()
rest = subprocess.Popen([sys.executable, f'{HERE}/mock_lmu_rest.py', '6397'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
ag = subprocess.Popen([BIN, '--key', 'arion_tk_' + 'ab' * 32, '--fake', '--fake-speed', '10', '--url', 'http://127.0.0.1:9', '--apikey', 'x', '--seconds', '150'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
time.sleep(1.5)
IDS = '[...document.querySelectorAll(".dcol")].map(c=>[...c.querySelectorAll("section.dock")].map(s=>s.dataset.id).join(",")).join(" | ")'
RECT = "(id=>{const e=document.querySelector(`section.dock[data-id=${id}]`);const r=(e.closest('.tgrp')||e).getBoundingClientRect();return {x:r.left,y:r.top,w:r.width,h:r.height,r:r.right,b:r.bottom,vis:!e.hidden}})"
async def drag(pg, frm, to_xy, tab=False, check=None):
    """drag panel `frm` by its title bar (or its tab) to a point; optionally read the drop-zone box just before letting go"""
    sel = f'.ttab[data-tab={frm}]' if tab else f'section.dock[data-id={frm}] .ph'
    await pg.evaluate(f"document.querySelector('{sel}').scrollIntoView({{block:'nearest'}})")
    b = await (await pg.query_selector(sel)).bounding_box()
    sx, sy = b['x'] + min(30, b['width'] / 2), b['y'] + b['height'] / 2
    await pg.mouse.move(sx, sy); await pg.mouse.down(); await pg.mouse.move(sx + 8, sy + 8, steps=2)
    await pg.mouse.move(to_xy[0], to_xy[1], steps=10); await asyncio.sleep(0.15)
    z = await pg.evaluate("(()=>{const z=document.querySelector('.dropz'),l=document.querySelector('.dropln');const f=e=>e&&e.style.display==='block'?(r=>({x:r.left,y:r.top,w:r.width,h:r.height}))(e.getBoundingClientRect()):null;return {zone:f(z),side:z&&z.dataset.side,line:f(l)}})()")
    await pg.mouse.up(); await asyncio.sleep(0.6)
    return z
async def main():
    async with async_playwright() as p:
        br = await p.chromium.launch(); errs = []
        ctx = await br.new_context(viewport={'width': 1920, 'height': 1080})
        pg = await ctx.new_page(); pg.on('pageerror', lambda e: errs.append(str(e)[:200]))
        await pg.route(lambda u: not u.startswith('http://127.0.0.1'), lambda r: r.abort())
        # an old (v0.1.13-style) saved layout is converted, not lost
        old = {'v': 1, 'cols': [{'w': 30, 'p': [['map', 60], ['inputs', 40]]}, {'w': 40, 'p': [['car', 50], ['laps', 50]]}, {'w': 30, 'p': [['moments', 100]]}]}
        await pg.add_init_script(f"if(!sessionStorage.getItem('seeded')){{localStorage.setItem('pw-layout-v4',{json.dumps(json.dumps(old))});sessionStorage.setItem('seeded','1')}}")
        await pg.goto('http://127.0.0.1:6420/'); await asyncio.sleep(12)
        E = pg.evaluate
        chk('dashboard 0.1.21', await E('VERSION') == '0.1.21')
        conv = await E(IDS); saved = await E('JSON.parse(localStorage.getItem("pw-layout-v5")||"null")')
        chk('an old saved layout is converted (same panels, same order) and saved in the new form', conv == 'map,inputs | car,laps | moments' and saved and saved['v'] == 2, [conv, saved])
        await pg.click('#gear'); await asyncio.sleep(0.2); await pg.click('#lreset'); await asyncio.sleep(0.8)
        chk('Reset: George’s default layout, exactly as before', await E(IDS) == 'car,damage,wx,inputs | map,battle | traffic,laps,pit,moments', await E(IDS))
        # ---- 1. split: Battle onto the right half of Live inputs → two columns under the map side… (George's picture)
        r = await E(RECT + "('inputs')")
        z = await drag(pg, 'battle', (r['x'] + r['w'] * 0.85, r['y'] + r['h'] * 0.5))
        chk('drag: the right half of Live inputs lights up', z['zone'] and z['side'] == 'right' and abs(z['zone']['x'] - (r['x'] + r['w'] / 2)) < 2 and abs(z['zone']['w'] - r['w'] / 2) < 2, [z, r])
        a, b = await E(RECT + "('inputs')"), await E(RECT + "('battle')")
        chk('split: Live inputs and Battle side by side, same height, in the left column', abs(a['y'] - b['y']) < 1 and abs(a['h'] - b['h']) < 1 and b['x'] > a['r'] and await E("!!document.querySelector('.dcol .lrow')"), [a, b])
        chk('split: Battle left the map column', await E(IDS) == 'car,damage,wx,inputs,battle | map | traffic,laps,pit,moments', await E(IDS))
        # top / bottom halves stack
        r = await E(RECT + "('traffic')")
        z = await drag(pg, 'moments', (r['x'] + r['w'] * 0.5, r['y'] + r['h'] * 0.1))
        chk('top half: Key moments goes above Traffic', z['side'] == 'top' and await E(IDS) == 'car,damage,wx,inputs,battle | map | moments,traffic,laps,pit', [z, await E(IDS)])
        # ---- tabs: Pit window onto the centre of Laps and sectors
        r = await E(RECT + "('laps')")
        z = await drag(pg, 'pit', (r['x'] + r['w'] * 0.5, r['y'] + r['h'] * 0.5))
        t = await E("(()=>{const g=document.querySelector('.tgrp');if(!g)return null;return {tabs:[...g.querySelectorAll('.ttab')].map(b=>[b.dataset.tab,b.classList.contains('on')]),shown:[...g.querySelectorAll(':scope>section.dock')].filter(s=>!s.hidden).map(s=>s.dataset.id)}})()")
        chk('centre: the whole panel lights up (dashed) for a tab', z['side'] == 'tab' and z['zone'] and abs(z['zone']['w'] - r['w']) < 2, z)
        chk('tabs: Laps and sectors + Pit window in one panel, the dropped one shown', t and t['tabs'] == [['laps', False], ['pit', True]] and t['shown'] == ['pit'], t)
        await pg.click('.ttab[data-tab=laps]'); await asyncio.sleep(0.5)
        t2 = await E("[[...document.querySelectorAll('.tgrp>section.dock')].filter(s=>!s.hidden).map(s=>s.dataset.id),document.querySelectorAll('#laps tr').length]")
        chk('tabs: a click switches (Laps shown, with its rows), Pit window hidden', t2[0] == ['laps'] and t2[1] >= 3, t2)
        chk('tabs: the panel title moves into the tab (no doubled «Laps and sectors»)', await E("getComputedStyle(document.querySelector('section.dock[data-id=laps] .ph>b')).display") == 'none')
        # map into a tab and back: the canvas must have its full size when shown
        r = await E(RECT + "('laps')")
        await drag(pg, 'map', (r['x'] + r['w'] * 0.5, r['y'] + r['h'] * 0.5))
        cv = await E("(()=>{const c=document.querySelector('#map'),r=c.getBoundingClientRect();return [c.width,Math.round(r.width*devicePixelRatio),r.width]})()")
        chk('tabs: the map works inside a tab (canvas at full size and sharp)', cv[2] > 200 and abs(cv[0] - cv[1]) <= 2, cv)
        await pg.click('.ttab[data-tab=pit]'); await asyncio.sleep(0.3); await pg.click('.ttab[data-tab=map]'); await asyncio.sleep(0.5)
        cv = await E("(()=>{const c=document.querySelector('#map'),r=c.getBoundingClientRect();return [c.width,Math.round(r.width*devicePixelRatio),r.width]})()")
        chk('tabs: switching back to the map redraws it at full size', cv[2] > 200 and abs(cv[0] - cv[1]) <= 2, cv)
        # a tab dragged out onto another panel's side leaves the group
        r = await E(RECT + "('car')")
        await drag(pg, 'map', (r['x'] + r['w'] * 0.5, r['y'] + r['h'] * 0.9), tab=True)
        chk('a tab dragged out leaves the group (Laps + Pit stay as tabs)', await E("[...document.querySelectorAll('.tgrp .ttab')].map(b=>b.dataset.tab).join()") == 'laps,pit' and await E("!document.querySelector('section.dock[data-id=map]').closest('.tgrp')"), await E(IDS))
        # ---- 2. a gutter between columns → a new column right there
        n0 = await E("document.querySelectorAll('.dcol').length")
        g = await (await pg.query_selector('#view>.gx')).bounding_box()
        z = await drag(pg, 'damage', (g['x'] + g['width'] / 2, g['y'] + 300))
        n1 = await E("document.querySelectorAll('.dcol').length"); cols = await E(IDS)
        chk('gutter: a bar lights up on it while dragging', z['line'] and z['line']['h'] > 200, z)
        chk('gutter: dropping there makes a new column at that place', n1 == n0 + 1 and cols.split(' | ')[1] == 'damage', [n0, n1, cols])
        # a gutter inside a split (between Live inputs and Battle) → a new column inside that split
        g = await (await pg.query_selector('.lrow>.gx')).bounding_box()
        await drag(pg, 'wx', (g['x'] + g['width'] / 2, g['y'] + g['height'] / 2))
        row = await E("[...document.querySelector('.lrow').children].filter(e=>!e.classList.contains('gx')).map(e=>e.dataset.id||e.querySelector('section.dock')?.dataset.id)")
        chk('inner gutter: Weather lands between Live inputs and Battle', row == ['inputs', 'wx', 'battle'], row)
        # gutter resize inside a split with room to move (Key moments beside the map), kept after reload
        r = await E(RECT + "('map')")
        await drag(pg, 'moments', (r['x'] + r['w'] * 0.9, r['y'] + r['h'] * 0.5))
        g = await (await pg.query_selector('.lrow:has(section.dock[data-id=map])>.gx')).bounding_box()
        w0 = (await E(RECT + "('map')"))['w']
        await pg.mouse.move(g['x'] + 5, g['y'] + 40); await pg.mouse.down(); await pg.mouse.move(g['x'] + 85, g['y'] + 40, steps=6); await pg.mouse.up(); await asyncio.sleep(0.4)
        w1 = (await E(RECT + "('map')"))['w']
        chk('inner gutter resizes the two sides (map wider by the drag)', 70 <= w1 - w0 <= 90, [w0, w1])
        # a split inside a narrow column can't squeeze panels below their minimum width
        mins = await E("[...document.querySelectorAll('.dcol:first-of-type .lrow>section.dock')].map(s=>[s.dataset.id,Math.round(s.getBoundingClientRect().width)])")
        chk('panels in a split never go below their minimum width', all(w >= {'inputs': 380, 'wx': 360, 'battle': 470}.get(i, 0) - 1 for i, w in mins), mins)
        lay = await E('localStorage.getItem("pw-layout-v5")'); ids = await E(IDS)
        await pg.reload(); await asyncio.sleep(5)
        chk('everything is remembered after a reload (layout, tabs, sizes)', await E('localStorage.getItem("pw-layout-v5")') == lay and await E(IDS) == ids and abs((await E(RECT + "('map')"))['w'] - w1) < 2, [ids, await E(IDS)])
        # Escape while dragging cancels; dropping a panel on itself does nothing
        before = await E('localStorage.getItem("pw-layout-v5")')
        b = await (await pg.query_selector('section.dock[data-id=traffic] .ph')).bounding_box()
        await pg.mouse.move(b['x'] + 30, b['y'] + 8); await pg.mouse.down(); await pg.mouse.move(b['x'] + 300, b['y'] + 200, steps=8); await pg.keyboard.press('Escape'); await pg.mouse.up(); await asyncio.sleep(0.5)
        r = await E(RECT + "('traffic')")
        await drag(pg, 'traffic', (r['x'] + r['w'] * 0.2, r['y'] + r['h'] * 0.5))
        chk('Escape cancels a drag; dropping on itself changes nothing', await E('localStorage.getItem("pw-layout-v5")') == before)
        # the menu still hides / shows panels in the new layout, Reset still restores the default
        await pg.click('#gear'); await asyncio.sleep(0.2); await pg.click('#setPop input[data-p=pit]'); await asyncio.sleep(0.5)
        chk('hiding a tab from ⚙ leaves a single panel (no group of one)', not await E("!!document.querySelector('.tgrp')") and not await E("!!document.querySelector('section.dock[data-id=pit]')"))
        await pg.click('#setPop input[data-p=pit]'); await asyncio.sleep(0.5)
        chk('showing it again puts it back', await E("!!document.querySelector('section.dock[data-id=pit]')"))
        await pg.click('#lreset'); await asyncio.sleep(0.6)
        chk('Reset restores George’s default after all that', await E(IDS) == 'car,damage,wx,inputs | map,battle | traffic,laps,pit,moments' and not await E("!!document.querySelector('.lrow,.tgrp')"))
        chk('no page errors', not errs, errs)
        await br.close()
try: asyncio.run(main())
finally:
    ag.terminate(); rest.terminate()
print(f'\n{ok} passed, {fail} failed')

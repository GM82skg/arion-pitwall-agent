"""Driver app v0.1.18 / dashboard v0.1.13: lists in columns (Traffic with car and VE), Full grid by default,
Panels menu (show / hide), Damage without the drawing."""
import asyncio, subprocess, time, sys, json
from playwright.async_api import async_playwright
BIN = sys.argv[1]; HERE = __file__.rsplit('/', 1)[0]; ok = fail = 0
def chk(n, c, info=''):
    global ok, fail
    if c: ok += 1; print('PASS', n)
    else: fail += 1; print('FAIL', n, json.dumps(info, ensure_ascii=False)[:500])
# every row's cells start at the same x as the header's (a real column layout)
ALIGN = '''(sel=>{const t=document.querySelector(sel);if(!t)return null;const rows=[...t.querySelectorAll("tr")].filter(r=>r.cells.length>1);
  const xs=rows.map(r=>[...r.cells].map(c=>Math.round(c.getBoundingClientRect().left)).join());return {n:rows.length,cols:rows[0]?rows[0].cells.length:0,same:new Set(xs).size===1}})'''
IDS = '[...document.querySelectorAll(".dcol")].map(c=>[...c.querySelectorAll("section.dock")].map(s=>s.dataset.id).join(",")).join(" | ")'
import portwait; portwait.wait_free()   # the previous test's app may still hold 6420
rest = subprocess.Popen([sys.executable, f'{HERE}/mock_lmu_rest.py', '6397'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
ag = subprocess.Popen([BIN, '--key', 'arion_tk_' + 'ab' * 32, '--fake', '--fake-speed', '10', '--url', 'http://127.0.0.1:9', '--apikey', 'x', '--seconds', '120'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
time.sleep(1.5)
async def main():
    async with async_playwright() as p:
        br = await p.chromium.launch(); errs = []
        pg = await (await br.new_context(viewport={'width': 1920, 'height': 1080})).new_page(); pg.on('pageerror', lambda e: errs.append(str(e)[:200]))
        await pg.route(lambda u: not u.startswith('http://127.0.0.1'), lambda r: r.abort())
        await pg.goto('http://127.0.0.1:6420/'); await asyncio.sleep(14)
        chk('dashboard version 0.1.13', await pg.evaluate('VERSION') == '0.1.13')
        # ---------- 2 ----------
        chk('Laps and sectors opens on Full grid', await pg.evaluate('S.gridTab') == 'all' and await pg.evaluate('document.querySelector("#gAll").className') == 'on' and await pg.evaluate('document.querySelectorAll("#laps tr.ch").length') >= 1)
        # ---------- 1 ----------
        hd = await pg.evaluate('[...document.querySelectorAll("#traffic tr.th th")].map(t=>t.innerText.trim())')
        chk('Traffic has columns: class · P · driver · car · VE · gap', hd[1:6] == ['P', 'Driver', 'Car', 'VE', 'Gap'], hd)
        tr = await pg.evaluate(ALIGN + '("#traffic table")')
        chk('Traffic: every row lines up with the header (≥ 5 rows)', tr and tr['same'] and tr['n'] >= 6, tr)
        cells = await pg.evaluate('[...document.querySelectorAll("#traffic tr.tr")].map(r=>[...r.cells].map(c=>c.innerText.trim()))')
        chk('Traffic: car and VE filled in (Car #NN, NN%)', all(c[3].startswith('Car #') and c[4].split('%')[0].isdigit() for c in cells), cells[:3])
        chk('Traffic: you say «you», the others a signed gap', any(c[5] == 'you' for c in cells) and all(c[5] == 'you' or c[5][0] in '+−' for c in cells), [c[5] for c in cells])
        await pg.evaluate('moment("★","Test moment for the column check","");paintMoments()')
        for sel in ['#laps table', '#battle table', '#moments table']:
            a = await pg.evaluate(ALIGN + f'("{sel}")')
            chk(f'{sel.split()[0]}: rows line up in columns', a and a['same'] and a['n'] >= 1, a)
        # ---------- 4 ----------
        dm = await pg.evaluate('({svg:!!document.querySelector("#dmg svg.carsvg"),txt:document.querySelector("#dmg").innerText,ph:document.querySelector("#dmg .ph").innerText})')
        chk('Damage: no car drawing', not dm['svg'], dm)
        chk('Damage: repair beside aero and body (not in the title bar)', all(x in dm['txt'] for x in ['Aero', 'Body', 'Repair', '12.4 s', 'Suspension', 'FL']) and 'repair' not in dm['ph'].lower(), dm)
        h = await pg.evaluate('Math.round(document.querySelector(".dock[data-id=damage]").getBoundingClientRect().height)')
        chk('Damage: shorter still (was 125 px)', h < 125, h)
        # ---------- 3 ----------
        await pg.click('#pnl'); await asyncio.sleep(0.3)
        boxes = await pg.evaluate('[...document.querySelectorAll("#pnlPop input")].map(c=>[c.dataset.p,c.checked])')
        chk('Panels: a tick per panel, all ticked', len(boxes) == 10 and all(b[1] for b in boxes), boxes)
        await pg.click('#pnlPop input[data-p=traffic]'); await pg.click('#pnlPop input[data-p=wx]'); await asyncio.sleep(0.6)
        ids = await pg.evaluate(IDS)
        chk('Panels: untick hides (Traffic, Weather gone; the rest keep their places)', ids == 'car,damage,inputs | map,battle | laps,pit,moments', ids)
        await pg.reload()
        for _ in range(16):
            await asyncio.sleep(0.5)
            if await pg.evaluate('document.querySelectorAll(".dcol section.dock").length'): break
        chk('Panels: kept after a reload', await pg.evaluate(IDS) == ids, [await pg.evaluate(IDS), await pg.evaluate('document.querySelector("#view").innerText.slice(0,100)')])
        await pg.click('#pnl'); await asyncio.sleep(0.3)
        await pg.click('#pnlPop input[data-p=wx]'); await pg.click('#pnlPop input[data-p=traffic]'); await asyncio.sleep(0.6)
        ids2 = await pg.evaluate(IDS)
        chk('Panels: tick shows again, back in its own place', ids2 == 'car,damage,wx,inputs | map,battle | traffic,laps,pit,moments', ids2)
        # hide all but one: the last can't be hidden; a column with nothing left disappears
        for pid in ['car', 'damage', 'wx', 'inputs', 'map', 'battle', 'traffic', 'laps', 'pit']:
            await pg.click(f'#pnlPop input[data-p={pid}]'); await asyncio.sleep(0.2)
        last = await pg.evaluate('({ids:' + IDS + ',dis:document.querySelector("#pnlPop input[data-p=moments]").disabled,cols:document.querySelectorAll(".dcol").length})')
        chk('Panels: the last panel can\'t be hidden; empty columns disappear', last['ids'] == 'moments' and last['dis'] and last['cols'] == 1, last)
        await pg.click('#pnlPop input[data-p=map]'); await asyncio.sleep(0.6)
        chk('Panels: a panel shown again gets its own column back', await pg.evaluate(IDS) in ('map | moments', 'moments | map') and await pg.evaluate('!!document.querySelector("#map")'), await pg.evaluate(IDS))
        await pg.keyboard.press('Escape'); await asyncio.sleep(0.2)
        chk('Panels: Escape closes the menu', not await pg.evaluate('!!document.querySelector("#pnlPop")'))
        await pg.click('#lreset'); await asyncio.sleep(0.6)
        chk('Reset layout shows every panel again', await pg.evaluate(IDS) == 'car,damage,wx,inputs | map,battle | traffic,laps,pit,moments')
        ph = await (await br.new_context(viewport={'width': 390, 'height': 844}, is_mobile=True, has_touch=True)).new_page()
        await ph.route(lambda u: not u.startswith('http://127.0.0.1'), lambda r: r.abort())
        await ph.goto('http://127.0.0.1:6420/'); await asyncio.sleep(3)
        chk('phones: no Panels button (tabs instead)', not await ph.evaluate('!!document.querySelector("#pnl").getClientRects().length'))
        chk('no page errors', not errs, errs)
        await br.close()
asyncio.run(main())
ag.terminate(); rest.terminate()
print(f'\n{ok} passed, {fail} failed')

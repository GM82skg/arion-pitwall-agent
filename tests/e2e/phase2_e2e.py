"""Phase 2 panels on My dashboard: fake car at 10x speed + LMU web interface stand-in."""
import asyncio, subprocess, time, sys
from playwright.async_api import async_playwright
BIN = sys.argv[1]; HERE = __file__.rsplit('/', 1)[0]; ok = fail = 0
def chk(n, c, info=''):
    global ok, fail
    if c: ok += 1; print('PASS', n)
    else: fail += 1; print('FAIL', n, info)
rest = subprocess.Popen([sys.executable, f'{HERE}/mock_lmu_rest.py', '6397'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
ag = subprocess.Popen([BIN, '--key', 'arion_tk_' + 'ab' * 32, '--fake', '--fake-speed', '10', '--url', 'http://127.0.0.1:9', '--apikey', 'x', '--seconds', '60'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
time.sleep(1.5)
async def main():
    async with async_playwright() as p:
        br = await p.chromium.launch(); errs = []
        pg = await (await br.new_context(viewport={'width': 1700, 'height': 1000})).new_page()
        pg.on('pageerror', lambda e: errs.append(str(e)[:200]))
        await pg.route(lambda u: not u.startswith('http://127.0.0.1'), lambda r: r.abort())
        await pg.goto('http://127.0.0.1:6420/'); await asyncio.sleep(22)
        T = lambda sel: pg.evaluate(f'(document.querySelector("{sel}")||{{}}).innerText||""')
        car = await T('#car'); chk('damage % from LMU web data, with repair time', 'Aero 14%' in car and 'Susp FL 6%' in car and 'repair 12.4 s' in car, car[-160:])
        chk('energy shows LMU maximum', 'of 100' in car, car[:200])
        chk('brake discs in mm', 'disc 32.0 mm' in car, car)
        wx = await T('#wx'); slots = await pg.evaluate('document.querySelectorAll("#wx .slot").length')
        chk('weather: 5 forecast slots for this session', slots == 5, wx[:200])
        chk('weather: next change with time and lap', 'rain chance' in wx and ' min' in wx, wx[:300])
        laps = await pg.evaluate('document.querySelectorAll("#laps tr").length')
        chk('laps and sectors: this car’s laps listed', laps >= 3, laps)
        secs = await pg.evaluate('[...document.querySelectorAll("#laps tr")].slice(1,2).map(r=>r.innerText)[0]||""')
        chk('three sector times per lap', secs.count('.') >= 4, secs)
        await pg.click('#gAll'); await asyncio.sleep(0.6)
        cls = await T('#gcls'); rows = await pg.evaluate('document.querySelectorAll("#laps tr").length')
        chk('full grid: class filter and your class preselected', 'GT3' in cls and 'All' in cls and rows == 6, (cls, rows))
        await pg.click('#gcls button[data-c="All"]'); await asyncio.sleep(0.4)
        chk('full grid: All shows every car', await pg.evaluate('document.querySelectorAll("#laps tr").length') == 9)
        bt = await T('#battle'); brow = await pg.evaluate('document.querySelectorAll("#battle tr").length')
        chk('battle in class: you plus the cars around you in class (here P1, so only the car behind)', brow == 3 and 'you' in bt, bt[:300])
        chk('battle: pace and energy filled', bt.count(':') >= 3 and '%' in bt, bt[:300])
        pw = await T('#pitw'); chk('pit window: LMU estimate and rejoin', 'Rejoin P' in pw and '34.6 s' in pw and '12.4 s' in pw, pw)
        await pg.screenshot(path='/tmp/phase2.png')
        chk('no page errors', not errs, errs)
        await br.close()
asyncio.run(main())
ag.terminate(); rest.terminate()
print(f'\n{ok} passed, {fail} failed')

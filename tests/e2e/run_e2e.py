"""End-to-end: Rust agent (fake car) ⇄ mock database ⇄ 6 headless-Chrome coaches.
Expect 5 coaches streaming at ~30 Hz with the right frame layout and a measured round trip, the 6th refused."""
import asyncio, subprocess, time, sys, os, json, statistics
from playwright.async_api import async_playwright
HERE = os.path.dirname(os.path.abspath(__file__)); PORT = 8399
BIN = sys.argv[1]
ok = fail = 0
def chk(n, c, info=''):
    global ok, fail
    if c: ok += 1; print('PASS', n)
    else: fail += 1; print('FAIL', n, info)
import portwait; portwait.wait_free()   # the previous test's app may still hold 6420
db = subprocess.Popen([sys.executable, f'{HERE}/mock_db.py', str(PORT)])
time.sleep(0.8)
bad = subprocess.run([BIN, '--key', 'arion_tk_' + 'cd' * 32, '--fake', '--url', f'http://127.0.0.1:{PORT}', '--apikey', 'test_pub', '--seconds', '3'], capture_output=True, text=True, timeout=30)
chk('a wrong key is reported, not crashed', 'Unknown key.' in bad.stdout, bad.stdout[-300:])
agent = subprocess.Popen(env={**os.environ, 'ARION_DEBUG': '1'}, args=[BIN, '--key', 'arion_tk_' + 'ab' * 32, '--fake', '--url', f'http://127.0.0.1:{PORT}', '--apikey', 'test_pub', '--seconds', '40'],
                         stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
time.sleep(1.5)
async def main():
    async with async_playwright() as p:
        br = await p.chromium.launch()  # default Chrome: local addresses hidden behind .local names, as for real coaches
        pages = []
        for i in range(1, 7):
            pg = await (await br.new_context()).new_page()
            await pg.goto(f'http://127.0.0.1:{PORT}/page?viewer=00000000-0000-0000-0000-00000000000{i}')
            pages.append(pg)
            await asyncio.sleep(0.4)
        await asyncio.sleep(14)
        R = [await pg.evaluate('window.R') for pg in pages]
        live = [r for r in R if r['state'] == 'connected']
        chk('5 coaches connected', len(live) == 5, [(r['viewer'][-1], r['state'], r.get('error')) for r in R])
        chk('the 6th coach is refused as full', R[5]['state'] == 'refused' and R[5]['bye'] == {'reason': 'full'}, R[5]['bye'])
        for r in live:
            ts = r['times'][-60:]
            hz = (len(ts) - 1) / ((ts[-1] - ts[0]) / 1000) if len(ts) > 2 else 0
            chk(f"coach {r['viewer'][-1]}: fast frames ~30 Hz ({hz:.1f})", 25 <= hz <= 35, hz)
            chk(f"coach {r['viewer'][-1]}: cars frames arrive", r['cars'] >= 50, r['cars'])
            chk(f"coach {r['viewer'][-1]}: slow messages arrive", r['slow'] >= 15, r['slow'])
            chk(f"coach {r['viewer'][-1]}: round trip measured", len(r['rtt']) >= 10, len(r['rtt']))
        f = live[0]['first']
        chk('fast frame is 76 bytes', f['len'] == 76, f)
        chk('fast frame values are the fake car', 21 < f['speed'] < 51 and 4000 < f['rpm'] < 8000 and f['gear'] in (3, 5) and abs(f['y']) < 1e-6, f)
        chk('brake pressure front/rear split when braking (or zero)', (f['bp'][0] == 0) or abs(f['bp'][0] / max(f['bp'][2], 1e-9) - 0.542 / 0.458) < 1e-3, f['bp'])
        c = live[0]['lastCars']; chk('cars frame: 8 cars, 4+16 bytes each', c['n'] == 8 and c['len'] == 4 + 16 * 8 and sorted(c['ids']) == list(range(1, 9)), c)
        s = live[0]['lastSlow']
        chk('slow message: session, car, grid', s['t'] == 'slow' and s['session']['track'] == 'Test Ring' and len(s['grid']) == 8 and len(s['car']['wheels']) == 4, list(s.keys()))
        chk('no SteamID anywhere in what coaches receive', 'steam' not in json.dumps(s).lower())
        rtts = [x for r in live for x in r['rtt']]
        print(f'round trip on this machine: median {statistics.median(rtts):.1f} ms, max {max(rtts):.1f} ms')
        # one coach leaves: the others keep streaming
        before = [r['fast'] for r in R[1:5]]
        await pages[0].close(); await asyncio.sleep(4)
        after = [await pg.evaluate('window.R.fast') for pg in pages[1:5]]
        chk('one coach leaving does not stop the others', all(a > b + 60 for a, b in zip(after, before)), (before, after))
        await br.close()
asyncio.run(main())
out, _ = agent.communicate(timeout=60)
calls = json.loads(subprocess.run(['curl', '-s', f'http://127.0.0.1:{PORT}/calls'], capture_output=True, text=True).stdout)
chk('the app says bye when it stops', calls[-1] == 'bye', calls[-3:])
polls = out.count('call poll'); chk(f'poll rate within the database limit ({polls} polls in ~40 s; limit 120/min)', 15 <= polls <= 25, polls)
chk('the app says hello once and bye once', out.count('call hello') == 1 and out.count('call bye') == 1)
print([l for l in out.strip().splitlines() if not l.startswith('call ')][-1])
db.terminate()
print(f'\n{ok} passed, {fail} failed')

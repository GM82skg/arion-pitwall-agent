"""My dashboard: the app serves the dashboard on 127.0.0.1 and streams to it locally (no database, no login)."""
import asyncio, subprocess, time, sys, os
from playwright.async_api import async_playwright
BIN = sys.argv[1]; ok = fail = 0
def chk(n, c, info=''):
    global ok, fail
    if c: ok += 1; print('PASS', n)
    else: fail += 1; print('FAIL', n, info)
# the key is checked against a database that isn't there: "My dashboard" must still work
import portwait; portwait.wait_free()   # the previous test's app may still hold 6420
ag = subprocess.Popen([BIN, '--key', 'arion_tk_' + 'ab' * 32, '--fake', '--url', 'http://127.0.0.1:9', '--apikey', 'x', '--seconds', '40'], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
url = None; t0 = time.time()
while time.time() - t0 < 10 and not url:
    l = ag.stdout.readline()
    if l.startswith('My dashboard: '): url = l.split(': ', 1)[1].strip()
chk('the app tells where My dashboard is', url == 'http://127.0.0.1:6420/', url)
bad = subprocess.run(['curl', '-s', '-o', '/dev/null', '-w', '%{http_code}', '-H', 'Host: evil.example:6420', 'http://127.0.0.1:6420/'], capture_output=True, text=True).stdout
chk('a request for another host is refused (403)', bad == '403', bad)
async def main():
    async with async_playwright() as p:
        br = await p.chromium.launch(); errs = []
        pg = await (await br.new_context(viewport={'width': 1600, 'height': 900})).new_page()
        pg.on('pageerror', lambda e: errs.append(str(e)[:200]))
        await pg.route(lambda u: not u.startswith('http://127.0.0.1'), lambda r: r.abort())  # no internet needed
        await pg.goto(url); await asyncio.sleep(6)
        i = await pg.evaluate('({local:LOCAL, tabs:document.querySelector("#tabs").innerText, link:document.querySelector("#link").innerText, login:!!document.querySelector("#lf"), hist:S.hist.length, cars:S.cars.size, slow:!!S.slow, segs:S.segs.length, car:document.querySelector("#car").innerText})')
        chk('local mode, no sign-in form', i['local'] and not i['login'], i)
        chk('tab: My car on this PC', 'My car' in i['tabs'], i['tabs'])
        chk('connection line: live · this PC', 'live' in i['link'] and 'this PC' in i['link'], i['link'])
        chk('live data arrives (inputs, cars, slow data, map)', i['hist'] > 100 and i['cars'] == 8 and i['slow'] and i['segs'] > 20, i)
        chk('car card filled', 'Best lap' in i['car'] and 'FL' in i['car'], i['car'][:120])
        await pg.screenshot(path='/tmp/local_dash.png')
        chk('no page errors', not errs, errs)
        await br.close()
asyncio.run(main())
ag.terminate()
print(f'\n{ok} passed, {fail} failed')

"""Driver app v0.1.21 / dashboard v0.1.16: one settings icon (⚙) at the top right holds size, panels and Reset layout;
your class position on the track map (class colour, extra bold; no overall); Live inputs title and colour key under the graph (v0.1.19: only the colour key; the title bar is back on top)."""
import asyncio, subprocess, time, sys
from playwright.async_api import async_playwright
BIN = sys.argv[1]; HERE = __file__.rsplit('/', 1)[0]; ok = fail = 0
def chk(n, c, info=''):
    global ok, fail
    if c: ok += 1; print('PASS', n)
    else: fail += 1; print('FAIL', n, info)
import portwait; portwait.wait_free()
rest = subprocess.Popen([sys.executable, f'{HERE}/mock_lmu_rest.py', '6397'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
ag = subprocess.Popen([BIN, '--key', 'arion_tk_' + 'ab' * 32, '--fake', '--fake-speed', '10', '--url', 'http://127.0.0.1:9', '--apikey', 'x', '--seconds', '75'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
time.sleep(1.5)
# what the badge should say, worked out here from the same data
WANT = """(()=>{const j=S.slow,me=j&&j.grid?j.grid.find(g=>g.player):null;if(!me)return {txt:'',col:null};
  const inCls=j.grid.filter(g=>g.class===me.class).sort((a,b)=>a.place-b.place),pc=inCls.findIndex(g=>g.id===me.id)+1;
  const t=document.createElement('i');t.style.color=CLS(me.class);document.body.appendChild(t);const col=getComputedStyle(t).color;t.remove();
  return {txt:'P'+pc,col}})()"""
GOT = """(()=>{const e=document.querySelector('#mpos');if(!e)return null;const b=e.querySelector('b'),cv=e.closest('.cv').getBoundingClientRect(),r=e.getBoundingClientRect();
  return {txt:e.textContent.replace(/\\s+/g,''),col:b?getComputedStyle(b).color:null,dr:Math.round(cv.right-r.right),db:Math.round(cv.bottom-r.bottom),inside:r.left>=cv.left&&r.top>=cv.top&&r.right<=cv.right&&r.bottom<=cv.bottom,
    fs:b?parseFloat(getComputedStyle(b).fontSize):0,fw:b?getComputedStyle(b).fontWeight:'',over:/overall/.test(e.textContent)}})()"""
def same(w, g): return g is not None and g['txt'] == w['txt'].replace(' ', '') and (w['col'] is None or g['col'] == w['col'])
async def main():
    async with async_playwright() as p:
        br = await p.chromium.launch(); errs = []
        pg = await (await br.new_context(viewport={'width': 1440, 'height': 900})).new_page(); pg.on('pageerror', lambda e: errs.append(str(e)[:200]))
        await pg.route(lambda u: not u.startswith('http://127.0.0.1'), lambda r: r.abort())
        await pg.goto('http://127.0.0.1:6420/'); await asyncio.sleep(14)
        E = pg.evaluate
        chk('dashboard 0.1.16 or later', await E('VERSION') in ('0.1.16', '0.1.17', '0.1.18', '0.1.19'))
        # ---------- 3: settings icon ----------
        top = await E("""(()=>{const vis=id=>{const e=document.getElementById(id);return !!e&&!!e.getClientRects().length};const g=document.querySelector('#gear').getBoundingClientRect(),t=document.querySelector('.top').getBoundingClientRect();
            return {gear:vis('gear'),pnl:vis('pnl'),lreset:vis('lreset'),zm:!!document.querySelector('.top .zm'),gr:Math.round(t.right-g.right),label:document.querySelector('#gear').getAttribute('aria-label')}})()""")
        chk('top bar: only the ⚙ icon, no Panels / Reset layout / size buttons', top['gear'] and not top['pnl'] and not top['lreset'] and not top['zm'] and top['label'] == 'Settings', top)
        chk('⚙ sits at the top right of the bar', top['gr'] <= 20, top)
        await pg.click('#gear'); await asyncio.sleep(0.3)
        m = await E("""(()=>{const d=document.querySelector('#setPop'),g=document.querySelector('#gear');if(!d)return null;const r=d.getBoundingClientRect(),gr=g.getBoundingClientRect();
            return {heads:[...d.querySelectorAll(':scope>b')].map(b=>b.textContent),zm:d.querySelector('#zmV')?.textContent,boxes:d.querySelectorAll('input[data-p]').length,reset:!!d.querySelector('#lreset'),
              under:r.top>=gr.bottom&&r.top<=gr.bottom+10,right:Math.round(gr.right-r.right),inwin:r.left>=0&&r.right<=innerWidth&&r.bottom<=innerHeight,
              on:g.classList.contains('on'),exp:g.getAttribute('aria-expanded')}})()""")
        chk('⚙ opens one menu: Zoom, Panels (10 ticks), Reset layout', m and m['heads'] == ['Zoom', 'Panels'] and m['zm'] == '100%' and m['boxes'] == 10 and m['reset'], m)
        chk('menu opens under the ⚙, right edges lined up, inside the window', m and m['under'] and abs(m['right']) <= 1 and m['inwin'], m)
        chk('⚙ shows it is open (yellow, aria-expanded)', m and m['on'] and m['exp'] == 'true', m)
        await pg.click('#zmIn'); await asyncio.sleep(0.5)
        m2 = await E("""(()=>{const d=document.querySelector('#setPop'),g=document.querySelector('#gear');if(!d)return null;const r=d.getBoundingClientRect(),gr=g.getBoundingClientRect();
            return {zm:d.querySelector('#zmV').textContent,z:getComputedStyle(document.documentElement).getPropertyValue('--z').trim(),under:r.top>=gr.bottom-1&&r.top<=gr.bottom+10,right:Math.round(gr.right-r.right)}})()""")
        chk('zoom + works inside the menu; the menu stays open and stays under the ⚙ after zooming', m2 and m2['zm'] == '110%' and m2['z'] == '1.1' and m2['under'] and abs(m2['right']) <= 1, m2)
        await pg.click('#zmV'); await asyncio.sleep(0.3)
        await pg.click('#gear'); await asyncio.sleep(0.2)
        chk('clicking ⚙ again closes it', not await E('!!document.querySelector("#setPop")') and await E('document.querySelector("#gear").getAttribute("aria-expanded")') == 'false')
        await pg.click('#gear'); await asyncio.sleep(0.2); await pg.mouse.click(700, 500); await asyncio.sleep(0.2)
        chk('a click outside closes it', not await E('!!document.querySelector("#setPop")'))
        await pg.click('#gear'); await asyncio.sleep(0.2); await pg.keyboard.press('Escape'); await asyncio.sleep(0.2)
        chk('Escape closes it, and the ⚙ is no longer yellow', not await E('!!document.querySelector("#setPop")') and not await E('document.querySelector("#gear").classList.contains("on")'))
        await pg.click('#gear'); await asyncio.sleep(0.2); await pg.click('#setPop input[data-p=damage]'); await asyncio.sleep(0.5)
        chk('a panel can be hidden from the menu, menu stays open', not await E('!!document.querySelector(".dock[data-id=damage]")') and await E('!!document.querySelector("#setPop")'))
        await pg.click('#lreset'); await asyncio.sleep(0.6)
        chk('Reset layout (in the menu) brings the panel back and closes the menu', await E('!!document.querySelector(".dock[data-id=damage]")') and not await E('!!document.querySelector("#setPop")'))
        # ---------- 4: position on the map ----------
        w = await E(WANT); g = await E(GOT)
        chk('map: class position shown, same as the data, nothing about overall', same(w, g) and not g['over'], [w, g])
        chk('map: in the class colour, large, extra bold (800), bottom right inside the map', g and g['fs'] >= 30 and g['fw'] == '800' and g['inside'] and 0 <= g['dr'] <= 14 and 0 <= g['db'] <= 12, g)
        print('    badge:', g['txt'] if g else None)
        chk('map: the position check catches a wrong number (planted)', not same(w, dict(g, txt='P9')) if g else False)
        # change the position and the class count, by hand
        r = await E("""(()=>{const keep=S.slow,j=JSON.parse(JSON.stringify(keep)),me=j.grid.find(g=>g.player);
            j.grid.forEach(g=>{if(g.class===me.class&&g.id!==me.id&&g.place>me.place){g.place=me.place;}});me.place=me.place+1;
            S.slow=j;paintMapPos();const a=document.querySelector('#mpos').textContent;
            const one=JSON.parse(JSON.stringify(j));one.grid=one.grid.filter(g=>g.class===me.class);S.slow=one;paintMapPos();const b=document.querySelector('#mpos').textContent;
            const none=JSON.parse(JSON.stringify(j));none.grid.forEach(g=>g.player=false);S.slow=none;paintMapPos();const c=document.querySelector('#mpos').textContent;
            S.slow=keep;paintMapPos();return {a,b,c,over:b.includes('overall')}})()""")
        chk('map: one class only → still just «P…»', r['b'].startswith('P') and not r['over'], r)
        chk('map: no player car → nothing shown', r['c'] == '', r)
        # ---------- Live inputs (v0.1.19): title bar on top (drag handle), only the colour key under the graph ----------
        li = await E("""(()=>{const d=document.querySelector('.dock[data-id=inputs]'),ph=d.querySelector(':scope>.ph'),cv=d.querySelector(':scope>.cv').getBoundingClientRect(),top=d.querySelector(':scope>.inptop').getBoundingClientRect(),k=d.querySelector(':scope>.ikey'),r=ph.getBoundingClientRect(),kr=k?k.getBoundingClientRect():null,dr=d.getBoundingClientRect();
            return {titleTop:r.bottom<=top.top+0.5&&r.top>=dr.top-0.5,title:ph.innerText.replace(/\\s+/g,' ').trim(),keyBelow:!!kr&&kr.top>=cv.bottom-0.5&&kr.bottom<=dr.bottom+0.5,key:k?k.innerText.replace(/\\s+/g,' ').trim():'',numsTop:top.bottom<=cv.top+0.5}})()""")
        chk('Live inputs: title bar on top (above the speed line), colour key alone under the graph, inside the panel', li['titleTop'] and li['title'] == 'Live inputs' and li['keyBelow'] and li['key'] == 'throttle brake steering ABS lock' and li['numsTop'], li)
        await E("document.querySelector('.dock[data-id=inputs]>.ph').scrollIntoView({block:'center'})"); await asyncio.sleep(0.3)   # the left column scrolls at 1440×900
        hb = await (await pg.query_selector('.dock[data-id=inputs]>.ph')).bounding_box()
        await pg.mouse.move(hb['x'] + 30, hb['y'] + 8); await pg.mouse.down(); await pg.mouse.move(hb['x'] + 60, hb['y'] - 200, steps=6)
        drag = await E("document.querySelector('.dock[data-id=inputs]').classList.contains('dragging')"); await pg.keyboard.press('Escape'); await pg.mouse.up(); await asyncio.sleep(0.5)
        chk('Live inputs: dragged by its title bar (top again)', drag)
        await pg.click('#gear'); await asyncio.sleep(0.2); await pg.click('#lreset'); await asyncio.sleep(0.6)
        chk('no page errors (desktop)', not errs, errs)
        # ---------- phone ----------
        ctx = await br.new_context(viewport={'width': 390, 'height': 844}, is_mobile=True, has_touch=True, device_scale_factor=2)
        ph = await ctx.new_page(); perr = []; ph.on('pageerror', lambda e: perr.append(str(e)[:200]))
        await ph.route(lambda u: not u.startswith('http://127.0.0.1'), lambda r: r.abort())
        await ph.goto('http://127.0.0.1:6420/'); await asyncio.sleep(6)
        chk('phone: no ⚙ (tabs instead)', not await ph.evaluate('!!document.querySelector("#gear").getClientRects().length'))
        await ph.click('.pnav button[data-t="map"]'); await asyncio.sleep(1.5)
        w = await ph.evaluate(WANT); g = await ph.evaluate(GOT)
        chk('phone Map tab: position shown at once, bottom right inside the map', same(w, g) and g['inside'] and g['fs'] >= 26, [w, g])
        await ph.screenshot(path='/tmp/v0121_phone_map.png')
        pi = await ph.evaluate("""(()=>{const p=document.querySelector('.pinp'),ph=p.querySelector(':scope>.ph').getBoundingClientRect(),cv=p.querySelector(':scope>.cv').getBoundingClientRect(),ft=p.querySelector(':scope>#inpft').getBoundingClientRect();return {below:ph.top>=cv.bottom-0.5&&ph.top>=ft.bottom-0.5}})()""")
        chk('phone Map tab: inputs colour key at the bottom', pi['below'], pi)
        chk('no page errors (phone)', not perr, perr)
        await pg.screenshot(path='/tmp/v0121_desktop.png')
        await pg.click('#gear'); await asyncio.sleep(0.3); await pg.screenshot(path='/tmp/v0121_menu.png', clip={'x': 1000, 'y': 0, 'width': 440, 'height': 560})
        await br.close()
try: asyncio.run(main())
finally:
    ag.terminate(); rest.terminate()
print(f'\n{ok} passed, {fail} failed')

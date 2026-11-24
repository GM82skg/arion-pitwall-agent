"""Driver app v0.1.16 / dashboard v0.1.11: F zoom − / +, G packing, H steady arrow and calmer trail colours."""
import asyncio, subprocess, time, sys, json
from playwright.async_api import async_playwright
BIN = sys.argv[1]; HERE = __file__.rsplit('/', 1)[0]; ok = fail = 0
def chk(n, c, info=''):
    global ok, fail
    if c: ok += 1; print('PASS', n)
    else: fail += 1; print('FAIL', n, json.dumps(info, ensure_ascii=False)[:500])
# empty space under each panel's contents (px)
EMPTY = '''Object.fromEntries([...document.querySelectorAll("section.dock")].map(d=>{const r=d.getBoundingClientRect(),cs=getComputedStyle(d);let b=r.top;
  const w=document.createTreeWalker(d,NodeFilter.SHOW_TEXT);let t;while(t=w.nextNode()){if(!t.textContent.trim()||t.parentElement.closest("svg"))continue;const g=document.createRange();g.selectNodeContents(t);const q=g.getBoundingClientRect();if(q.height>0)b=Math.max(b,q.bottom)}
  for(const e of d.querySelectorAll("svg,canvas,img,button,.bar,.tyre,.slot"))if(!e.parentElement.closest("svg")&&e.getClientRects().length){const q=e.getBoundingClientRect();if(q.height>0)b=Math.max(b,q.bottom)}
  return [d.dataset.id,Math.round(r.bottom-parseFloat(cs.paddingBottom)-b)]}))'''
import portwait; portwait.wait_free()   # the previous test's app may still hold 6420
rest = subprocess.Popen([sys.executable, f'{HERE}/mock_lmu_rest.py', '6397'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
ag = subprocess.Popen([BIN, '--key', 'arion_tk_' + 'ab' * 32, '--fake', '--fake-speed', '10', '--url', 'http://127.0.0.1:9', '--apikey', 'x', '--seconds', '120'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
time.sleep(1.5)
async def main():
    async with async_playwright() as p:
        br = await p.chromium.launch(); errs = []
        ctx = await br.new_context(viewport={'width': 1920, 'height': 1080})
        pg = await ctx.new_page(); pg.on('pageerror', lambda e: errs.append(str(e)[:200]))
        await pg.route(lambda u: not u.startswith('http://127.0.0.1'), lambda r: r.abort())
        await pg.goto('http://127.0.0.1:6420/'); await asyncio.sleep(14)
        chk('version 0.1.21', await pg.evaluate('VERSION') == '0.1.21')
        # ---------- G ----------
        lay = await pg.evaluate('[...document.querySelectorAll(".dcol")].map(c=>[...c.querySelectorAll("section.dock")].map(s=>s.dataset.id).join(","))')
        chk('G: new default layout: Live inputs under the left column, the map gets the middle, Pit window on the right', lay == ['car,damage,wx,inputs', 'map,battle', 'traffic,laps,pit,moments'], lay)
        ov = await pg.evaluate('[document.documentElement.scrollHeight,innerHeight,...[...document.querySelectorAll(".dcol")].map(c=>c.scrollHeight-c.clientHeight)]')
        chk('G: everything fits the window at 1920x1080 (no page or column overflow)', ov[0] <= ov[1] + 1 and all(x <= 1 for x in ov[2:]), ov)
        e = await pg.evaluate(EMPTY)
        chk('G: no empty space under Car, Damage, Weather, Pit window, Battle (≤ 12 px)', all(e[k] <= 12 for k in ['car', 'damage', 'wx', 'pit', 'battle']), e)
        sl = await pg.evaluate('[...document.querySelectorAll("#wx .slot")].map(s=>Math.round(s.getBoundingClientRect().height))')
        chk('G: weather tiles as tall as their contents (were 119 px)', sl and max(sl) <= 90, sl)
        dh = await pg.evaluate('Math.round(document.querySelector(".dock[data-id=damage]").getBoundingClientRect().height)')
        chk('G: Damage panel shorter (was 200 px)', dh <= 165, dh)
        dg = await pg.evaluate('document.querySelector("#dmg").innerText')
        chk('G: Damage still shows aero, body and all 4 suspension corners', all(x in dg for x in ['Aero', 'Body', 'Suspension', 'FL', 'FR', 'RL', 'RR']), dg)
        pt = await pg.evaluate('document.querySelector("#pitw").innerText')
        chk('G: Pit window shows the stop plan (must stop · VE · tyres)', 'Must stop' in pt and 'lap' in pt and 'tyres' in pt, pt)
        bt = await pg.evaluate('document.querySelector("#battle").innerText')
        chk('G: Battle never says «to close the 999 s» for a car a lap ahead', '999' not in bt, bt)
        # ---------- H ----------
        await pg.click('#mFollow'); await asyncio.sleep(1.5)
        h = await pg.evaluate('''new Promise(res=>{const out=[];const cv=document.querySelector("#map");let n=0;
          const step=()=>{const mp=mePos(),T=viewTransform(cv,mp),p=T.f(mp.x,mp.z);   /* one position per frame, as drawMap does */
            const c=[...S.cars.values()].find(c=>c.me);let old=null;if(c){const k=Math.max(0,Math.min(1,(performance.now()-c.t)/Math.max(30,c.t-c.pt)));old=T.f(c.px+(c.x-c.px)*k,c.pz+(c.z-c.pz)*k)}
            out.push({p,old,hs:S.hs,raw:S.heading,t:performance.now()/1000,W:cv.width,H:cv.height});if(++n<120)requestAnimationFrame(step);else res(out)};requestAnimationFrame(step)})''')
        dev = max(max(abs(x['p'][0] - x['W'] / 2), abs(x['p'][1] - x['H'] * 0.62)) for x in h)
        old = max((((x['old'][0] - x['p'][0]) ** 2 + (x['old'][1] - x['p'][1]) ** 2) ** 0.5) for x in h if x['old'])
        chk('H: in Follow the arrow stays on its spot (≤ 0.5 px)', dev <= 0.5, dev)
        chk('H: …where the old way (10 Hz car position in a 30 Hz view) wandered (> 2 px)', old > 2, old)
        import math
        # smooth = no spikes in the turn rate (its frame-to-frame change), compared with turning straight on the raw heading
        def spikes(k):   # turn rate in rad/s from real frame times, then its biggest jump between frames
            t = [((h[i][k] - h[i - 1][k] + math.pi) % (2 * math.pi) - math.pi) / max(1e-3, h[i]['t'] - h[i - 1]['t']) for i in range(1, len(h))]
            return max(abs(t[i] - t[i - 1]) for i in range(1, len(t)))
        sm, rw = spikes('hs'), spikes('raw')
        chk('H: the view turns without jerks (turn-rate jumps ≤ 1/3 of turning on the raw heading)', sm * 3 <= rw, [sm, rw])
        await pg.click('#mFull'); await asyncio.sleep(0.5)
        # v0.1.20 (George, choice A): the trail is coloured by the pedals, no longer by the delta (that check moved to the
        # dashboard repo's tests/v0120_e2e.py); here only that the H drawing still runs with it
        fl = await pg.evaluate('pedalColours([[0,0,0,1,0],[0,0,0,0,1],[0,0,0,0,0]])')
        chk('H: trail colour by pedals: throttle green, brake red, none grey', fl == ['#3BD16F', '#FF5A5A', '#9A9DA6'], fl)
        # ---------- F ----------
        await pg.click('#gear'); await asyncio.sleep(0.3)   # v0.1.16: size is in the settings menu
        chk('F: − 100% + in the settings menu (⚙)', await pg.evaluate('document.querySelector("#setPop #zmV").innerText') == '100%')
        await pg.click('#zmIn'); await pg.click('#zmIn'); await asyncio.sleep(0.6)
        z = await pg.evaluate('({v:document.querySelector("#zmV").innerText,z:getComputedStyle(document.documentElement).getPropertyValue("--z"),s:localStorage.getItem("pw-zoom"),w:document.querySelector("#car .ph b").getBoundingClientRect().height})')
        chk('F: + + → 120 %, applied and remembered', z['v'] == '120%' and z['z'].strip() == '1.2' and z['s'] == '1.2', z)
        a = await pg.evaluate('(()=>{const r=document.querySelector("#app").getBoundingClientRect();return [Math.round(r.width),Math.round(r.height),innerWidth,innerHeight,document.documentElement.scrollHeight<=innerHeight+1]})()')
        chk('F: the dashboard still fills the window exactly at 120 %', abs(a[0] - a[2]) <= 2 and abs(a[1] - a[3]) <= 2 and a[4], a)
        cv = await pg.evaluate('(()=>{const c=document.querySelector("#map"),r=c.getBoundingClientRect();return [c.width,Math.round(r.width*devicePixelRatio)]})()')
        chk('F: the map canvas keeps full sharpness when zoomed', abs(cv[0] - cv[1]) <= 2, cv)
        for _ in range(8):
            if await pg.evaluate('document.querySelector("#zmIn").disabled'): break
            await pg.click('#zmIn')
        chk('F: stops at 150 %', await pg.evaluate('document.querySelector("#zmV").innerText') == '150%' and await pg.evaluate('document.querySelector("#zmIn").disabled'))
        await pg.click('#zmV'); await asyncio.sleep(0.3)
        chk('F: clicking the value goes back to 100 %', await pg.evaluate('document.querySelector("#zmV").innerText') == '100%' and await pg.evaluate('localStorage.getItem("pw-zoom")') == '1')
        await pg.evaluate('localStorage.setItem("pw-zoom","1.3")'); await pg.reload(); await asyncio.sleep(3); await pg.click('#gear'); await asyncio.sleep(0.3)
        chk('F: a reload keeps the size', await pg.evaluate('document.querySelector("#zmV").innerText') == '130%' and await pg.evaluate('getComputedStyle(document.documentElement).getPropertyValue("--z").trim()') == '1.3')
        await pg.keyboard.press('Escape'); await asyncio.sleep(0.2)   # close the settings menu again (v0.1.16)
        ph = await (await br.new_context(viewport={'width': 390, 'height': 844}, is_mobile=True, has_touch=True)).new_page()
        await ph.route(lambda u: not u.startswith('http://127.0.0.1'), lambda r: r.abort())
        await ph.add_init_script('localStorage.setItem("pw-zoom","1.4")')
        await ph.goto('http://127.0.0.1:6420/'); await asyncio.sleep(3)
        r = await ph.evaluate('({vis:!!document.querySelector("#gear").getClientRects().length,z:getComputedStyle(document.documentElement).getPropertyValue("--z").trim()})')
        chk('F: phones: no buttons and no zoom (they pinch)', not r['vis'] and r['z'] == '1', r)
        ap = await pg.evaluate('(()=>{paintAppV({app:{v:"0.1.16",new:"0.1.17"}});const a=document.querySelector("#appv").innerText;paintAppV({app:{v:"0.1.16",new:null}});return [a,document.querySelector("#appv").innerText]})()')
        chk('coaches see «driver app v0.1.16 · v0.1.17 available», and nothing when up to date', ap == ['driver app v0.1.16 · v0.1.17 available', ''], ap)
        # ---------- standings (v0.1.12) on a hand-made grid: GT3 sorts before HY by name, but HY is faster ----------
        st = await pg.evaluate('''(()=>{const keep={slow:S.slow,cars:S.cars,L:S.trackLen,tab:S.gridTab,cls:S.gridCls};
          try{const g=(id,name,cls,place,laps,bl,best,last,extra)=>({id,name,car:'Car #'+(10+id),class:cls,place,laps,behindLeader:bl,best,last,pits:false,player:false,s1:0,s2:0,...(extra||{})});
            S.slow={...S.slow,grid:[g(1,'Zed GT','GT3',3,40,60.2,96.1,96.5,{player:true}),g(2,'Amy GT','GT3',4,40,63.0,95.8,96.9,{pits:true}),g(3,'Bob GT','GT3',5,39,140,96.4,97.0),
              g(4,'Hyp One','Hypercar',1,41,0,80.1,80.4),g(5,'Hyp Two','Hypercar',2,41,1.9,80.3,80.6)]};
            S.trackLen=1000;S.cars=new Map([[1,{dist:500}],[2,{dist:470}],[3,{dist:450}],[4,{dist:600}],[5,{dist:580}]].map(([i,c])=>[i,{id:i,...c}]));
            S.gridTab='all';S.gridCls='All';paintLaps();
            const rows=[...document.querySelectorAll('#laps tr')].map(r=>[r.className,...[...r.cells].map(c=>c.innerText.trim())]);
            const pb=[...document.querySelectorAll('#laps .pb')].map(e=>e.innerText);return {rows,pb}}
          finally{S.slow=keep.slow;S.cars=keep.cars;S.trackLen=keep.L;S.gridTab=keep.tab;S.gridCls=keep.cls;paintLaps()}})()''')
        R = st['rows']
        chk('standings: fastest class first, with a header (Hypercar 2 cars, then GT3 3 cars)', R[1][1].startswith('Hypercar') and '2 cars' in R[1][1] and R[4][1].startswith('GT3') and '3 cars' in R[4][1], R)
        chk('standings: position in class, car number and driver', [r[1] for r in R[2:4]] == ['1', '2'] and [r[1] for r in R[5:8]] == ['1', '2', '3'] and R[5][2].startswith('#11') and 'Zed GT' in R[5][2], R)
        chk('standings: gap to the class leader, «–» for the leader, «1L» when lapped', R[2][3] == '–' and R[3][3] == '1.9' and R[6][3] == '2.8' and R[7][3] == '1L', R)
        chk('standings: a car in the pits says PIT', 'PIT' in R[6][2] and 'PIT' not in R[5][2], R)
        chk('standings: fastest and last lap; only each class best in purple', R[5][4] == '1:36.100' and R[5][5] == '1:36.500' and st['pb'] == ['1:20.100', '1:35.800'], [R[5], st['pb']])
        chk('standings: your row highlighted', R[5][0] == 'me', R[5])
        await pg.click('#gAll'); await asyncio.sleep(0.6)
        lg1 = await pg.evaluate('[...document.querySelectorAll(".lsec")].map(e=>getComputedStyle(e).display)')
        await pg.click('#gMine'); await asyncio.sleep(0.6)
        lg2 = await pg.evaluate('[...document.querySelectorAll(".lsec")].map(e=>getComputedStyle(e).display)')
        chk('standings: only «class best» in the legend; This car shows all three again', lg1 == ['none', 'none'] and 'none' not in lg2 and len(lg2) == 2, [lg1, lg2])
        chk('no page errors', not errs, errs)
        await br.close()
asyncio.run(main())
ag.terminate(); rest.terminate()
print(f'\n{ok} passed, {fail} failed')

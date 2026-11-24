"""v0.1.15 / dashboard v0.1.10: B live delta, ghost and corner verdicts · C OBS panels · D strategy alerts.
Runs the driver app with the fake race (10x) and LMU's web interface stand-in, then checks My dashboard."""
import asyncio, subprocess, time, sys, json
from playwright.async_api import async_playwright
BIN = sys.argv[1]; HERE = __file__.rsplit('/', 1)[0]; ok = fail = 0
def chk(n, c, info=''):
    global ok, fail
    if c: ok += 1; print('PASS', n)
    else: fail += 1; print('FAIL', n, json.dumps(info, ensure_ascii=False)[:600])
PANELS = ['car', 'damage', 'wx', 'pit', 'map', 'inputs', 'battle', 'traffic', 'laps', 'moments']
# a race state made by hand, for the strategy model: you (GT3) and three rivals, laps of 100 s, 1000 m track
MODEL = r'''(sc)=>{const keep={per:S.per,cars:S.cars,rest:S.rest,trackLen:S.trackLen,lane:S.lane};
 try{const now=Date.now();S.per=new Map();S.cars=new Map();S.rest={pit:{total:30,repair:0}};S.trackLen=1000;S.lane=null;
  const mk=(id,name,car,place,laps,ageAtNow,base,slope,e,per,extra)=>{const st=laps-ageAtNow;
    const times=[];for(let l=Math.max(st+1,laps-6);l<=laps;l++)times.push({lap:l,t:base+slope*(l-st)});
    S.per.set(id,{laps,times,stintStart:st,stops:1,best:[9e9,9e9,9e9],ve:[{lap:laps-4,e:e+4*per},{lap:laps,e}],...(extra||{})});
    S.cars.set(id,{id,dist:(extra&&extra.dist)||500,x:0,z:0});
    return {id,name,car,class:'GT3',place,laps,energy:e,pits:false,pitState:0,player:false,behindLeader:0}};
  const g=sc.grid.map(a=>mk(...a));g[0].player=true;
  const j={t:'slow',session:{type:10,phase:5,yellow:0,left:sc.left??7200},car:{wheels:[{wear:0.6},{wear:0.9},{wear:0.62},{wear:0.88}]},grid:g};
  const r=stratModel(j);return r?{k:r.k,gain:r.gain,cover:r.cover&&r.cover.no,flipped:r.flipped.map(f=>f.no),ty:r.ty,ve:r.ve,rows:r.rows,est:r.est}:null}
 finally{Object.assign(S,keep)}}'''
import portwait; portwait.wait_free()   # the previous test's app may still hold 6420
rest = subprocess.Popen([sys.executable, f'{HERE}/mock_lmu_rest.py', '6397'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
ag = subprocess.Popen([BIN, '--key', 'arion_tk_' + 'ab' * 32, '--fake', '--fake-speed', '10', '--url', 'http://127.0.0.1:9', '--apikey', 'x', '--seconds', '150'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
time.sleep(1.5)
async def main():
    async with async_playwright() as p:
        br = await p.chromium.launch(); errs = []
        ctx = await br.new_context(viewport={'width': 1700, 'height': 1000})
        await ctx.grant_permissions(['clipboard-read', 'clipboard-write'], origin='http://127.0.0.1:6420')
        pg = await ctx.new_page(); pg.on('pageerror', lambda e: errs.append(str(e)[:200]))
        await pg.route(lambda u: not u.startswith('http://127.0.0.1'), lambda r: r.abort())
        await pg.goto('http://127.0.0.1:6420/'); await asyncio.sleep(1)
        chk('dashboard version 0.1.19', await pg.evaluate('VERSION') == '0.1.19')
        # ---------- B ----------
        st = None
        for _ in range(60):
            st = await pg.evaluate('({ref:!!S.ref,cmp:!!S.cmp,own:S.cmp&&S.cmp.own,v:S.verdicts.length,dlt:(document.querySelector("#dlt")||{}).innerText})')
            if st['ref'] and st['own'] is not None and st['v'] >= 2: break
            await asyncio.sleep(1)
        chk('B: class-best reference arrives (ref message)', st['ref'], st)
        chk('B: comparison frames arrive (kind 3) with an own-best delta', st['cmp'] and st['own'] is not None, st)
        ref = await pg.evaluate('({n:S.ref.corners.length,line:S.ref.line.length,lap:S.ref.lap,own:S.ref.own_lap,rev:S.ref.rev===S.cmp.rev})')
        # (at 10x the samples are 10x further apart; the unit test checks the full-rate line, >300 points)
        chk('B: reference has the 2 corners of the test track, a line, both lap times, same revision as the frames', ref['n'] == 2 and ref['line'] > 150 and ref['lap'] > 0 and ref['own'] >= ref['lap'] and ref['rev'], ref)
        d = await pg.evaluate('document.querySelector("#dlt").innerText')
        chk('B: delta row shows a number, projected and potential lap', any(c.isdigit() for c in d.split('proj')[0]) and 'proj' in d and 'pot' in d and ':' in d.split('proj')[1], d)
        chk('B: default is own best', await pg.evaluate('DMODE') == 'own' and await pg.evaluate('document.querySelector("[data-dm=own]").className') == 'on')
        own_d = await pg.evaluate('curDelta()')
        await pg.click('[data-dm=class]'); await asyncio.sleep(0.5)
        cls = await pg.evaluate('({m:DMODE,saved:localStorage.getItem("pw-delta"),d:curDelta(),c:S.cmp.d})')
        chk('B: switch to class best (saved, uses the class delta)', cls['m'] == 'class' and cls['saved'] == 'class' and abs(cls['d'] - cls['c']) < 1e-6, cls)
        chk('B: the fake player is slower against the class best than against itself', cls['d'] is not None and own_d is not None and cls['d'] > own_d - 0.5, [own_d, cls['d']])
        await pg.click('[data-dm=own]'); await asyncio.sleep(0.3)
        v = await pg.evaluate('S.verdicts.filter(x=>x.at).length?verdictText(S.verdicts.find(x=>x.at)):""')
        chk('B: corner verdicts arrive and read like «T1 +0.18 · braked 12 m early»', v.startswith('<span class="tn">T') and ('braked' in v or 'apex' in v or 'exit' in v), v)
        vis = await pg.evaluate('(document.querySelector("#dlt .vd")||{}).innerText||""')
        chk('B: the latest verdict is shown in the delta row', vis.startswith('T'), vis)
        # the map draws the ghost (purple ring) and the braking markers: count purple pixels on the canvas
        purple = await pg.evaluate('''(()=>{const c=document.querySelector("#map"),g=c.getContext("2d"),d=g.getImageData(0,0,c.width,c.height).data;let n=0;
            for(let i=0;i<d.length;i+=4)if(Math.abs(d[i]-0xC9)<8&&Math.abs(d[i+1]-0xA8)<8&&Math.abs(d[i+2]-0xFF)<8)n++;return n})()''')
        chk('B: ghost and braking markers drawn on the map (purple)', purple > 60, purple); print('    purple pixels', purple)
        await asyncio.sleep(1)   # switching the delta clears the trail's old colours on purpose
        cols = await pg.evaluate('(()=>{const t=S.trail.slice(-25);return [t.length,t.filter(p=>p[2]!=null).length]})()')
        chk('B: the trail carries the delta (for green/red)', cols[0] >= 20 and cols[1] >= cols[0] - 2, cols)
        # ---------- C ----------
        n = await pg.evaluate('document.querySelectorAll(".chain").length')
        chk('C: every panel has a chain icon (10)', n == 10, n)
        await pg.click('.dock[data-id=map] .chain'); await asyncio.sleep(0.3)
        chk('C: chain opens the OBS popup (background slider, preview, copy)', await pg.evaluate('!!document.querySelector("#obsPop #obsBg")&&!!document.querySelector("#obsPv")&&!!document.querySelector("#obsCopy")'))
        await pg.evaluate('(()=>{const r=document.querySelector("#obsBg");r.value=60;r.dispatchEvent(new Event("input"))})()')
        pv = await pg.evaluate('getComputedStyle(document.querySelector("#obsPv")).backgroundColor')
        chk('C: the preview shows the chosen opacity', pv == 'rgba(22, 23, 26, 0.6)', pv)
        await pg.click('#obsCopy'); await asyncio.sleep(0.4)
        clip = await pg.evaluate('navigator.clipboard.readText()')
        chk('C: copies the OBS link with the background (&bg=60) and closes', clip == 'http://127.0.0.1:6420/?panel=map&obs=1&bg=60' and not await pg.evaluate('!!document.querySelector("#obsPop")'), clip)
        await pg.click('.dock[data-id=car] .chain'); await asyncio.sleep(0.3)
        chk('C: the choice is remembered for the next panel', await pg.evaluate('document.querySelector("#obsBg").value') == '60' and await pg.evaluate('localStorage.getItem("pw-obsbg")') == '60')
        await pg.keyboard.press('Escape'); await asyncio.sleep(0.2)
        chk('C: Escape closes the popup', not await pg.evaluate('!!document.querySelector("#obsPop")'))
        note = await pg.evaluate('document.querySelector("#note").innerText')
        chk('C: and says so', 'OBS link copied' in note, note)
        o = await ctx.new_page(); o.on('pageerror', lambda e: errs.append('obs: ' + str(e)[:200]))
        await o.route(lambda u: not u.startswith('http://127.0.0.1'), lambda r: r.abort())
        await o.set_viewport_size({'width': 800, 'height': 500})
        for pid in PANELS:
            await o.goto(f'http://127.0.0.1:6420/?panel={pid}&obs=1'); await asyncio.sleep(2.5)
            r = await o.evaluate('''(()=>{const s=document.querySelectorAll("section.dock");const top=getComputedStyle(document.querySelector(".top")).display;
              const bg=[getComputedStyle(document.body).backgroundColor,getComputedStyle(document.documentElement).backgroundColor,s[0]&&getComputedStyle(s[0]).backgroundColor,s[0]&&getComputedStyle(s[0]).borderTopWidth];
              const vb=[...document.querySelectorAll("section.dock button")].filter(b=>b.getClientRects().length).length;
              return {n:s.length,id:s[0]&&s[0].dataset.id,top,bg,vb,txt:(s[0]||{}).innerText||""}})()''')
            chk(f'C: OBS {pid}: only that panel, no top bar, transparent, no frame, no buttons, has content',
                r['n'] == 1 and r['id'] == pid and r['top'] == 'none' and r['bg'][:2] == ['rgba(0, 0, 0, 0)'] * 2 and r['bg'][2] in ('rgba(0, 0, 0, 0)', 'rgba(22, 23, 26, 0)') and r['bg'][3] == '0px' and r['vb'] == 0 and (len(r['txt'].strip()) > 3 or pid in ('map', 'inputs')), r)
        await o.goto('http://127.0.0.1:6420/?panel=map&obs=1'); await asyncio.sleep(3)
        corner = await o.evaluate('(()=>{const c=document.querySelector("#map");return [...c.getContext("2d").getImageData(1,1,1,1).data]})()')
        chk('C: OBS map canvas is transparent (no dark fill)', corner[3] == 0, corner)
        await o.goto('http://127.0.0.1:6420/?panel=battle&obs=1&bg=60'); await asyncio.sleep(1.5)
        b60 = await o.evaluate('getComputedStyle(document.querySelector("section.dock")).backgroundColor')
        chk('C: &bg=60 gives the panel its colour at 60 %', b60 == 'rgba(22, 23, 26, 0.6)', b60)
        await o.goto('http://127.0.0.1:6420/?panel=map&obs=1&bg=60'); await asyncio.sleep(3)
        c60 = await o.evaluate('(()=>{const c=document.querySelector("#map");return [...c.getContext("2d").getImageData(1,1,1,1).data]})()')
        chk('C: …and the map behind the track too (alpha ≈ 60 %)', abs(c60[3] - 153) <= 2, c60)
        live = await o.evaluate('S.cars.size>0&&S.hist.length>10')
        chk('C: OBS page gets live data with no login', live)
        await o.close()
        # ---------- D ----------
        # 1) undercut: #31 is 1.5 s ahead on 20-lap-old tyres and must stop in 3 laps; you can run 6. Pitting now wins the place.
        # (tyres lose 0.2 s per lap of age; laps ~100 s, so 10 m of track ≈ 1 s)
        und = {'grid': [[1, 'You', 'Car #13', 2, 40, 20, 100, 0.2, 0.30, 0.05], [2, 'Rival A', 'Car #31', 1, 40, 20, 100, 0.2, 0.16, 0.05, {'dist': 515}], [3, 'Rival B', 'Car #46', 3, 40, 5, 102, 0.2, 0.60, 0.05, {'dist': 450}]]}
        r = await pg.evaluate(MODEL, und)
        chk('D: undercut → PIT NOW, +1, undercut #31, change left tyres', r and r['k'] == 0 and r['gain'] >= 1 and '#31' in r['flipped'] and r['ty'] == 'L' and r['est'], r)
        chk('D: every option played through (now, +1, +2, stay out)', r and [x['k'] for x in r['rows']] == [0, 1, 2, None], r and r['rows'])
        # 2) nothing to gain: you're alone at the front with fresh tyres
        r2 = await pg.evaluate(MODEL, {'grid': [[1, 'You', 'Car #13', 1, 40, 2, 100, 0.1, 0.60, 0.05], [2, 'Rival A', 'Car #31', 2, 40, 2, 101, 0.1, 0.60, 0.05, {'dist': 400}]]})
        chk('D: silent when a stop gains nothing', r2 is None, r2)
        # 3) cover, 25 min from the flag: #46, right behind you in class, stopped 20 s ago (fresh tyres, now 58 s behind);
        #    you are on 25-lap-old tyres and must stop in 8 laps. Staying out lets #46 by; pitting now keeps you ahead.
        cov = {'left': 1500, 'grid': [[1, 'You', 'Car #13', 2, 40, 25, 100, 0.1, 0.42, 0.05], [2, 'Rival A', 'Car #31', 1, 40, 2, 95, 0.1, 0.60, 0.05, {'dist': 800}],
               [3, 'Rival B', 'Car #46', 3, 39, 2, 100, 0.1, 0.95, 0.02, {'dist': 920, 'stopAt': int(time.time() * 1000) - 20000}]]}
        r3 = await pg.evaluate(MODEL, cov)
        chk('D: cover → COVER #46, pit now', r3 and r3['cover'] == '#46' and r3['k'] <= 1 and r3['gain'] >= 1, r3)
        # 4) under a yellow, in practice, or with no choice left it stays silent
        y = json.loads(json.dumps(und)); y['left'] = 150
        chk('D: silent when the race ends before the stop matters', await pg.evaluate(MODEL, y) is None)
        # 5) the banner: only after 3 equal answers in a row, then on the map, opens on click, adds a key moment
        b = await pg.evaluate('''(sc)=>{const r={k:0,gain:2,cover:null,flipped:[{no:'#31',stop:43},{no:'#46',stop:43}],rows:[{k:0,pos:3,d:2,best:true},{k:1,pos:4,d:1},{k:2,pos:5,d:0},{k:null,pos:5,d:null}],ve:0.78,ty:'L',repair:0,loss:59,est:true};
            const keep=window.stratModel;window.stratModel=()=>r;S.st=null;S.stHist=[];const j=S.slow;const out=[];
            for(let i=0;i<3;i++){stratStep(j);out.push(!!S.st)}window.stratModel=keep;paintStrat();return out}''', None)
        chk('D: shown only after the same answer 3 times', b == [False, False, True], b)
        await pg.evaluate('window.stratStep=()=>{}')   # hold the banner for the checks below
        t = await pg.evaluate('document.querySelector("#strat").innerText')
        chk('D: banner reads PIT NOW +2, undercut #31 · #46, VE 78%, tyres L, repair –', all(x in t for x in ['PIT NOW', '+2', 'undercut #31 · #46', 'VE 78%', 'tyres L', 'repair –']), t)
        chk('D: banner pulses (yellow)', await pg.evaluate('document.querySelector("#sbtn").classList.contains("now")'))
        await pg.click('#sbtn'); await asyncio.sleep(0.2)
        t2 = await pg.evaluate('document.querySelector("#strat").innerText')
        chk('D: click opens every option and the estimated loss', all(x in t2 for x in ['now', '+1 lap', '+2 laps', 'stay out', 'P3', 'loss 59 s', 'est.', '#31 #46 out of VE lap 43']), t2)
        chk('D: key moment written', await pg.evaluate('S.moments.some(m=>m.text.startsWith("PIT NOW: +2"))'))
        fl = await pg.evaluate('getComputedStyle(document.querySelector("#flags")).bottom')
        chk('D: flags move below while the banner shows', fl == '8px', fl)
        chk('no page errors', not errs, errs)
        await br.close()
asyncio.run(main())
ag.terminate(); rest.terminate()
print(f'\n{ok} passed, {fail} failed')

"""Text-fit audit: no text may spill out of its box (panel, card, chip, tab, button, tile) on phones
(each tab, upright and sideways) and on common desktop sizes. Clipping on purpose (… ellipsis) is allowed."""
import asyncio, subprocess, time, sys, json
from playwright.async_api import async_playwright
BIN = sys.argv[1]; HERE = __file__.rsplit('/', 1)[0]; ok = fail = 0
def chk(n, c, info=''):
    global ok, fail
    if c: ok += 1; print('PASS', n)
    else: fail += 1; print('FAIL', n, json.dumps(info, ensure_ascii=False)[:900])
# v0.1.10: every panel title bar stays one line on desktop (a child starting below the first row = wrapped)
HDR = '''[...document.querySelectorAll(".dock .ph")].filter(e=>{const rs=[...e.children].filter(c=>c.getClientRects().length&&c.offsetHeight>0&&c.offsetWidth>0).map(c=>c.getBoundingClientRect());const fb=Math.min(...rs.map(r=>r.bottom));return rs.some(r=>r.top>=fb-1)}).map(e=>e.closest(".dock").dataset.id)'''
AUDIT = r'''(()=>{const BOX='.pn,.tab,.chip,button,.slot,.tyre,.next,.pill,.sc,.rl,td,th,.pty>div,.pk>div';const out=[];
 const w=document.createTreeWalker(document.querySelector('#app'),NodeFilter.SHOW_TEXT);let n;
 while(n=w.nextNode()){const t=n.textContent.trim();if(!t)continue;const el=n.parentElement;if(!el||!el.getClientRects().length)continue;
  const cs=getComputedStyle(el);if(cs.visibility==='hidden'||cs.display==='none')continue;
  const r=document.createRange();r.selectNodeContents(n);const tr=r.getBoundingClientRect();if(!tr.width)continue;
  const box=el.closest(BOX)||el.closest('.pn');if(!box)continue;const dock=el.closest('.pn');
  let clip=el;let allowed=false;while(clip&&clip!==box.parentElement){const c=getComputedStyle(clip);if(c.textOverflow==='ellipsis'||clip.classList.contains('tabs')){allowed=true;break}clip=clip.parentElement}
  if(allowed)continue;const bb=box.getBoundingClientRect(),bc=getComputedStyle(box),zz=box.currentCSSZoom||1,pl=(parseFloat(bc.paddingLeft)+parseFloat(bc.borderLeftWidth))*zz,pr=(parseFloat(bc.paddingRight)+parseFloat(bc.borderRightWidth))*zz,pb=parseFloat(bc.borderBottomWidth)*zz;const b={left:bb.left+pl,right:bb.right-pr,bottom:bb.bottom-pb};
  const db=dock?dock.getBoundingClientRect():null;const vl=!!el.closest('.vlist');if(db&&((!vl&&tr.bottom>db.bottom+1)||tr.right>db.right+1))out.push({text:t.slice(0,40),box:'cut off by panel',over:Math.round(Math.max(tr.bottom-db.bottom,tr.right-db.right))});
  else if(tr.right>b.right+1||tr.left<b.left-1||tr.bottom>b.bottom+1)out.push({text:t.slice(0,40),box:(box.className||box.tagName).toString().slice(0,30),over:Math.round(Math.max(tr.right-b.right,b.left-tr.left,tr.bottom-b.bottom))})}
 // and every small box (cell, chip, tile, button) must itself stay inside its panel's content area
 for(const el of document.querySelectorAll('#app td,#app th,#app .chip,#app .tab,#app .slot,#app .tyre,#app .next,#app button,#app .pill,#app .rl,#app .pty>div,#app .pk>div,#app table')){
  if(!el.getClientRects().length)continue;const pn=el.parentElement&&el.parentElement.closest('.pn');if(!pn)continue;
  let p=el.parentElement,scroll=false;while(p&&p!==pn){if(p.classList.contains('tabs')){scroll=true;break}p=p.parentElement}if(scroll)continue;
  const r=el.getBoundingClientRect(),bb=pn.getBoundingClientRect(),bc=getComputedStyle(pn),zz=pn.currentCSSZoom||1,L=bb.left+(parseFloat(bc.paddingLeft)+parseFloat(bc.borderLeftWidth))*zz,R=bb.right-(parseFloat(bc.paddingRight)+parseFloat(bc.borderRightWidth))*zz;
  if(r.right>R+1||r.left<L-1)out.push({box:(el.className||el.tagName).toString().slice(0,30),text:el.innerText.trim().slice(0,30),over:Math.round(Math.max(r.right-R,L-r.left))})}
 for(const el of document.querySelectorAll('#app .body,#app .dock,#app .pview,#app .pn'))if(el.getClientRects().length&&el.scrollWidth>el.clientWidth+1)out.push({box:'sideways overflow in '+(el.id||el.className).toString().slice(0,30),over:el.scrollWidth-el.clientWidth});
 const sw=document.documentElement.scrollWidth>innerWidth;return {out,sw}})()'''
rest = subprocess.Popen([sys.executable, f'{HERE}/mock_lmu_rest.py', '6397'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
ag = subprocess.Popen([BIN, '--key', 'arion_tk_' + 'ab' * 32, '--fake', '--fake-speed', '10', '--url', 'http://127.0.0.1:9', '--apikey', 'x', '--seconds', '120'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
time.sleep(1.5)
async def main():
    async with async_playwright() as p:
        br = await p.chromium.launch()
        ctx = await br.new_context(viewport={'width': 390, 'height': 844}, is_mobile=True, has_touch=True, device_scale_factor=2)
        pg = await ctx.new_page(); await pg.route(lambda u: not u.startswith('http://127.0.0.1'), lambda r: r.abort())
        await pg.goto('http://127.0.0.1:6420/'); await asyncio.sleep(14)
        # the audit must catch a real overflow: put a long word into a small box and expect a finding
        probe = await pg.evaluate("(()=>{const d=document.createElement('div');d.className='pn';d.style.width='60px';d.textContent='ThisWordIsFarTooLongForTheBox';document.querySelector('.pview').appendChild(d);const a="+AUDIT+";d.remove();return a.out.length})()")
        chk('the audit itself catches a real overflow', probe >= 1, probe)
        chk('phone: four tabs (Inputs now under the map)', await pg.evaluate('[...document.querySelectorAll(".pnav button")].map(b=>b.dataset.t).join()') == 'car,map,timing,strat')
        # v0.1.10: audit the strategy banner opened with its longest content, and the delta row with a long corner verdict
        await pg.evaluate("""(()=>{window.stratStep=()=>{};S.stOpen=true;
          S.st={key:'x',since:Date.now(),k:0,gain:3,cover:{id:1,no:'#397'},flipped:[{no:'#146',stop:118},{no:'#31',stop:121},{no:'#7',stop:124}],
            rows:[{k:0,pos:3,d:3,best:true},{k:1,pos:4,d:2},{k:2,pos:5,d:1},{k:null,pos:6,d:null}],ve:1,ty:'FL RR',repair:128,loss:118,est:true};
          const v={t:'corner',n:12,lap:3,dt:-1.234,brake_m:-123,vmin:-18,vexit:+12,abs:true,lock:true,at:Date.now()+3e7};S.verdicts=[v];
          setInterval(()=>{S.verdicts[0]=v},200)})()""")
        for size in [(390, 844), (360, 740)]:
            await pg.set_viewport_size({'width': size[0], 'height': size[1]})
            for tab in ['car', 'map', 'timing', 'strat']:
                await pg.click(f'.pnav button[data-t="{tab}"]'); await asyncio.sleep(1.2)
                if tab == 'map':
                    chk(f'phone {size[0]}: map with live inputs under it', await pg.evaluate('!!document.querySelector("#map")&&!!document.querySelector("#inp")&&S.hist.length>10'))
                    chk(f'phone {size[0]}: the opened strategy banner is on the map', await pg.evaluate('!!document.querySelector("#strat .sbt")'))
                if tab == 'car': chk(f'phone {size[0]}: the delta row is on the Car tab', await pg.evaluate('!!document.querySelector("#pdl .dv")'))
                a = await pg.evaluate(AUDIT); chk(f'phone {size[0]} {tab}: no text outside its box', not a['out'] and not a['sw'], a)
                await pg.screenshot(path=f'/tmp/fit_{size[0]}_{tab}.png')
        await pg.click('.pnav button[data-t="map"]'); await pg.set_viewport_size({'width': 844, 'height': 390}); await asyncio.sleep(1.2)
        land = await pg.evaluate('(()=>{const m=document.querySelector(".pmap").getBoundingClientRect(),i=document.querySelector(".pinp").getBoundingClientRect();return [document.body.classList.contains("land"),Math.round(m.right)<=Math.round(i.left),Math.round(i.height)>250]})()')
        chk('phone sideways: map and inputs side by side, full screen', all(land), land)
        a = await pg.evaluate(AUDIT); chk('phone sideways: no text outside its box', not a['out'] and not a['sw'], a)
        await pg.screenshot(path='/tmp/fit_land.png')
        for size in [(1920, 1080), (1600, 900), (1280, 720), (1024, 768)]:
            await pg.set_viewport_size({'width': size[0], 'height': size[1]}); await asyncio.sleep(1.5)
            chk(f'desktop {size[0]}x{size[1]}: banner, delta row and 10 chain icons on screen', await pg.evaluate('!!document.querySelector("#strat .sbt")&&!!document.querySelector("#dlt .vd")&&document.querySelectorAll(".chain").length===10'))
            a = await pg.evaluate(AUDIT); chk(f'desktop {size[0]}x{size[1]}: no text outside its box', not a['out'], a)
            w = await pg.evaluate(HDR); chk(f'desktop {size[0]}x{size[1]}: every panel title bar on one line', not w, w)
            if size[0] == 1024:   # the check must catch a real wrap: put the zoom readout back in the narrow map
                pw = await pg.evaluate('(()=>{const st=document.createElement("style");st.textContent="#zv{display:inline!important}";document.head.appendChild(st);const r='+HDR+';st.remove();return r})()')
                chk('the title-bar check catches a real wrap', 'map' in pw, pw)
            await pg.screenshot(path=f'/tmp/fit_d{size[0]}.png')
        # v0.1.11: the dashboard zoom (− / +): the same audit at the smallest and biggest size on a 1920x1080 screen
        await pg.set_viewport_size({'width': 1920, 'height': 1080})
        for z in ['0.8', '1.5']:
            await pg.evaluate(f'setZoom({z})'); await asyncio.sleep(1.5)
            a = await pg.evaluate(AUDIT); chk(f'desktop 1920x1080 at {int(float(z)*100)} %: no text outside its box', not a['out'], a)
            w = await pg.evaluate(HDR); chk(f'desktop 1920x1080 at {int(float(z)*100)} %: every panel title bar on one line', not w, w)
            await pg.screenshot(path=f'/tmp/fit_z{z}.png')
        await pg.evaluate('setZoom(1)')
        # v0.1.12: the standings (Full grid · All) at the widest and narrowest desktop sizes
        await pg.click('#gAll'); await pg.click('#gcls button[data-c="All"]')
        for size in [(1920, 1080), (1024, 768)]:
            await pg.set_viewport_size({'width': size[0], 'height': size[1]}); await asyncio.sleep(1.5)
            chk(f'standings {size[0]}: class headers shown', await pg.evaluate('document.querySelectorAll("#laps tr.ch").length') >= 2)
            a = await pg.evaluate(AUDIT); chk(f'standings {size[0]}x{size[1]}: no text outside its box', not a['out'], a)
        await br.close()
asyncio.run(main())
ag.terminate(); rest.terminate()
print(f'\n{ok} passed, {fail} failed')

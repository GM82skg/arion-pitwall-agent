"""Text-fit audit: no text may spill out of its box (panel, card, chip, tab, button, tile) on phones
(each tab, upright and sideways) and on common desktop sizes. Clipping on purpose (… ellipsis) is allowed."""
import asyncio, subprocess, time, sys, json
from playwright.async_api import async_playwright
BIN = sys.argv[1]; HERE = __file__.rsplit('/', 1)[0]; ok = fail = 0
def chk(n, c, info=''):
    global ok, fail
    if c: ok += 1; print('PASS', n)
    else: fail += 1; print('FAIL', n, json.dumps(info, ensure_ascii=False)[:900])
AUDIT = r'''(()=>{const BOX='.pn,.tab,.chip,button,.slot,.tyre,.next,.pill,.sc,.rl,td,th,.pty>div,.pk>div';const out=[];
 const w=document.createTreeWalker(document.querySelector('#app'),NodeFilter.SHOW_TEXT);let n;
 while(n=w.nextNode()){const t=n.textContent.trim();if(!t)continue;const el=n.parentElement;if(!el||!el.getClientRects().length)continue;
  const cs=getComputedStyle(el);if(cs.visibility==='hidden'||cs.display==='none')continue;
  const r=document.createRange();r.selectNodeContents(n);const tr=r.getBoundingClientRect();if(!tr.width)continue;
  const box=el.closest(BOX)||el.closest('.pn');if(!box)continue;const dock=el.closest('.pn');
  let clip=el;let allowed=false;while(clip&&clip!==box.parentElement){const c=getComputedStyle(clip);if(c.textOverflow==='ellipsis'||clip.classList.contains('tabs')){allowed=true;break}clip=clip.parentElement}
  if(allowed)continue;const bb=box.getBoundingClientRect(),bc=getComputedStyle(box),pl=parseFloat(bc.paddingLeft)+parseFloat(bc.borderLeftWidth),pr=parseFloat(bc.paddingRight)+parseFloat(bc.borderRightWidth),pb=parseFloat(bc.borderBottomWidth);const b={left:bb.left+pl,right:bb.right-pr,bottom:bb.bottom-pb};
  const db=dock?dock.getBoundingClientRect():null;const vl=!!el.closest('.vlist');if(db&&((!vl&&tr.bottom>db.bottom+1)||tr.right>db.right+1))out.push({text:t.slice(0,40),box:'cut off by panel',over:Math.round(Math.max(tr.bottom-db.bottom,tr.right-db.right))});
  else if(tr.right>b.right+1||tr.left<b.left-1||tr.bottom>b.bottom+1)out.push({text:t.slice(0,40),box:(box.className||box.tagName).toString().slice(0,30),over:Math.round(Math.max(tr.right-b.right,b.left-tr.left,tr.bottom-b.bottom))})}
 // and every small box (cell, chip, tile, button) must itself stay inside its panel's content area
 for(const el of document.querySelectorAll('#app td,#app th,#app .chip,#app .tab,#app .slot,#app .tyre,#app .next,#app button,#app .pill,#app .rl,#app .pty>div,#app .pk>div,#app table')){
  if(!el.getClientRects().length)continue;const pn=el.parentElement&&el.parentElement.closest('.pn');if(!pn)continue;
  let p=el.parentElement,scroll=false;while(p&&p!==pn){if(p.classList.contains('tabs')){scroll=true;break}p=p.parentElement}if(scroll)continue;
  const r=el.getBoundingClientRect(),bb=pn.getBoundingClientRect(),bc=getComputedStyle(pn),L=bb.left+parseFloat(bc.paddingLeft)+parseFloat(bc.borderLeftWidth),R=bb.right-parseFloat(bc.paddingRight)-parseFloat(bc.borderRightWidth);
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
        for size in [(390, 844), (360, 740)]:
            await pg.set_viewport_size({'width': size[0], 'height': size[1]})
            for tab in ['car', 'map', 'timing', 'strat']:
                await pg.click(f'.pnav button[data-t="{tab}"]'); await asyncio.sleep(1.2)
                if tab == 'map': chk(f'phone {size[0]}: map with live inputs under it', await pg.evaluate('!!document.querySelector("#map")&&!!document.querySelector("#inp")&&S.hist.length>10'))
                a = await pg.evaluate(AUDIT); chk(f'phone {size[0]} {tab}: no text outside its box', not a['out'] and not a['sw'], a)
                await pg.screenshot(path=f'/tmp/fit_{size[0]}_{tab}.png')
        await pg.click('.pnav button[data-t="map"]'); await pg.set_viewport_size({'width': 844, 'height': 390}); await asyncio.sleep(1.2)
        land = await pg.evaluate('(()=>{const m=document.querySelector(".pmap").getBoundingClientRect(),i=document.querySelector(".pinp").getBoundingClientRect();return [document.body.classList.contains("land"),Math.round(m.right)<=Math.round(i.left),Math.round(i.height)>250]})()')
        chk('phone sideways: map and inputs side by side, full screen', all(land), land)
        a = await pg.evaluate(AUDIT); chk('phone sideways: no text outside its box', not a['out'] and not a['sw'], a)
        await pg.screenshot(path='/tmp/fit_land.png')
        for size in [(1920, 1080), (1600, 900), (1280, 720), (1024, 768)]:
            await pg.set_viewport_size({'width': size[0], 'height': size[1]}); await asyncio.sleep(1.5)
            a = await pg.evaluate(AUDIT); chk(f'desktop {size[0]}x{size[1]}: no text outside its box', not a['out'], a)
            await pg.screenshot(path=f'/tmp/fit_d{size[0]}.png')
        await br.close()
asyncio.run(main())
ag.terminate(); rest.terminate()
print(f'\n{ok} passed, {fail} failed')

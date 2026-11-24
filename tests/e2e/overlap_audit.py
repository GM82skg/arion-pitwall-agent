"""Overlap audit (v0.1.16): no control covers another, and a selected or grouped button shows its whole outline.
Desktop 1440 and 1920 wide (settings menu open and closed), phone 390 on every tab."""
import asyncio, subprocess, time, sys, json
from playwright.async_api import async_playwright
BIN = sys.argv[1]; HERE = __file__.rsplit('/', 1)[0]; ok = fail = 0
def chk(n, c, info=''):
    global ok, fail
    if c: ok += 1; print('PASS', n)
    else: fail += 1; print('FAIL', n, json.dumps(info)[:600] if info else '')
import portwait; portwait.wait_free()
rest = subprocess.Popen([sys.executable, f'{HERE}/mock_lmu_rest.py', '6397'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
ag = subprocess.Popen([BIN, '--key', 'arion_tk_' + 'ab' * 32, '--fake', '--fake-speed', '10', '--url', 'http://127.0.0.1:9', '--apikey', 'x', '--seconds', '80'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
time.sleep(1.5)
# 1) two visible controls / labels whose boxes cross by more than 1 px (a shared 1 px border is allowed)
# 2) a button whose own outline is covered: the point just inside each edge must belong to the button itself
AUDIT = """(()=>{const sel='button,input,select,a,.chip,.pill,.chain,#mpos b,.ph>b,.ph>span:not(.grow),.dsw,.zm';
  const vis=e=>{const r=e.getBoundingClientRect();if(r.width<2||r.height<2)return false;const cs=getComputedStyle(e);return cs.visibility!=='hidden'&&cs.display!=='none'&&+cs.opacity>0.05};
  const els=[...document.querySelectorAll(sel)].filter(vis);const name=e=>(e.id?'#'+e.id:e.tagName.toLowerCase()+(e.className&&typeof e.className==='string'?'.'+e.className.split(' ')[0]:''))+'«'+(e.innerText||e.value||'').trim().slice(0,14)+'»';
  const over=[];
  const menu=e=>!!e.closest('#setPop,.obspop');   // an open menu lies over the page on purpose
  for(let i=0;i<els.length;i++)for(let k=i+1;k<els.length;k++){const a=els[i],b=els[k];if(a.contains(b)||b.contains(a)||menu(a)!==menu(b))continue;
    const r=a.getBoundingClientRect(),q=b.getBoundingClientRect();const w=Math.min(r.right,q.right)-Math.max(r.left,q.left),h=Math.min(r.bottom,q.bottom)-Math.max(r.top,q.top);
    if(w>1.01&&h>1.01)over.push([name(a),name(b),Math.round(w),Math.round(h)])}
  const hidden=[];
  for(const b of document.querySelectorAll('button')){if(!vis(b))continue;const r=b.getBoundingClientRect(),cs=getComputedStyle(b);if(cs.borderTopWidth==='0px')continue;
    const pts=[[r.left+0.5,r.top+r.height/2],[r.right-0.5,r.top+r.height/2],[r.left+r.width/2,r.top+0.5],[r.left+r.width/2,r.bottom-0.5]];
    // in a group the two buttons share one 1 px line; the selected button must own it, the other may not
    const grp=b.parentElement&&b.parentElement.getAttribute('role')==='group',sel=b.classList.contains('on');
    for(const [x,y] of pts){if(x<0||y<0||x>=innerWidth||y>=innerHeight)continue;const t=document.elementFromPoint(x,y);if(!t||t===b||b.contains(t)||t.closest('#setPop,.obspop'))continue;
      if(grp&&!sel&&t.parentElement===b.parentElement)continue;hidden.push([name(b),name(t)]);break}}
  return {n:els.length,over,hidden}})()"""
async def run(pg, tag):
    r = await pg.evaluate(AUDIT)
    chk(f'{tag}: no controls covering each other ({r["n"]} checked)', not r['over'], r['over'])
    chk(f'{tag}: every button shows its whole outline', not r['hidden'], r['hidden'])
    return r
async def main():
    async with async_playwright() as p:
        br = await p.chromium.launch()
        for w, h in [(1440, 900), (1920, 1080)]:
            pg = await (await br.new_context(viewport={'width': w, 'height': h})).new_page()
            await pg.route(lambda u: not u.startswith('http://127.0.0.1'), lambda r: r.abort())
            await pg.goto('http://127.0.0.1:6420/'); await asyncio.sleep(10 if w == 1440 else 5)
            for mode in ['own', 'class']:
                await pg.click(f'[data-dm={mode}]'); await asyncio.sleep(0.4)
                await run(pg, f'{w} Δ {mode}')
            await pg.hover('[data-dm=class]'); await asyncio.sleep(0.2)
            await run(pg, f'{w} hovering a switch')
            await pg.click('#gear'); await asyncio.sleep(0.3)
            await run(pg, f'{w} settings menu open')
            await pg.keyboard.press('Escape')
            # planted: put the old rule back (no z-index) and the outline check must catch it
            await pg.click('[data-dm=own]'); await asyncio.sleep(0.3)
            r = await pg.evaluate("(()=>{const st=document.createElement('style');st.textContent='.dsw button.on,.dsw button:hover{z-index:auto!important}.dsw button{position:static!important}';document.head.appendChild(st);return st.id='plant'})()")
            await pg.mouse.move(5, 5); await asyncio.sleep(0.2)
            r = await pg.evaluate(AUDIT)
            chk(f'{w}: the outline check catches the old switch (planted)', any('«Δ own»' in x[0] for x in r['hidden']), r['hidden'])
            await pg.evaluate("document.getElementById('plant').remove()")
            await pg.context.close()
        ctx = await br.new_context(viewport={'width': 390, 'height': 844}, is_mobile=True, has_touch=True, device_scale_factor=2)
        ph = await ctx.new_page(); await ph.route(lambda u: not u.startswith('http://127.0.0.1'), lambda r: r.abort())
        await ph.goto('http://127.0.0.1:6420/'); await asyncio.sleep(5)
        for t in ['car', 'map', 'timing', 'strat']:
            await ph.click(f'.pnav button[data-t="{t}"]'); await asyncio.sleep(1)
            await run(ph, f'phone {t}')
        await br.close()
try: asyncio.run(main())
finally:
    ag.terminate(); rest.terminate()
print(f'\n{ok} passed, {fail} failed')

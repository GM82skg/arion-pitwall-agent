"""v0.1.16 updates, end to end against a stand-in for GitHub: check, download, checksum, swap, cleanup, port wait."""
import hashlib, http.server, json, os, shutil, socket, subprocess, sys, tempfile, threading, time
BIN = os.path.abspath(sys.argv[1]); ok = fail = 0
import re
CUR = re.search(r'^version = "([^"]+)"', open(os.path.join(os.path.dirname(__file__), '..', '..', 'Cargo.toml')).read(), re.M).group(1)   # this app's version
a, b, c = map(int, CUR.split('.')); OLDER = f'v{a}.{b}.{c - 1}' if c else f'v{a}.{b - 1}.99'
def chk(n, c, info=''):
    global ok, fail
    if c: ok += 1; print('PASS', n)
    else: fail += 1; print('FAIL', n, str(info)[:400])
NEW = b'#!/bin/sh\necho "new version $@"\n'
STATE = {'tag': 'v' + CUR, 'sum': hashlib.sha256(NEW).hexdigest(), 'with_sum': True, 'hits': 0}
class H(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def do_GET(self):
        base = f'http://127.0.0.1:{self.server.server_port}'
        if self.path == '/latest':
            STATE['hits'] += 1
            ua = self.headers.get('User-Agent', '')
            assets = [{'name': 'ARION-Pit-Wall-%s.exe' % STATE['tag'], 'browser_download_url': base + '/exe'}, {'name': 'ARION-Pit-Wall.exe', 'browser_download_url': base + '/exe'}]
            if STATE['with_sum']: assets.append({'name': 'ARION-Pit-Wall.exe.sha256', 'browser_download_url': base + '/sum'})
            body = json.dumps({'tag_name': STATE['tag'], 'html_url': base + '/page', 'assets': assets, 'ua': ua}).encode()
            STATE['ua'] = ua
        elif self.path == '/exe': body = NEW
        elif self.path == '/sum': body = (STATE['sum'].upper() + '  ARION-Pit-Wall.exe\n').encode()
        else: self.send_response(404); self.end_headers(); return
        self.send_response(200); self.send_header('Content-Length', str(len(body))); self.end_headers(); self.wfile.write(body)
srv = http.server.ThreadingHTTPServer(('127.0.0.1', 0), H); threading.Thread(target=srv.serve_forever, daemon=True).start()
env = {**os.environ, 'ARION_UPDATE_API': f'http://127.0.0.1:{srv.server_port}/latest'}
d = tempfile.mkdtemp(); exe = os.path.join(d, f'ARION-Pit-Wall-v{CUR}.exe')
def fresh(): shutil.copy(BIN, exe); [os.remove(os.path.join(d, f)) for f in os.listdir(d) if f.endswith(('.old', '.new'))]
def run(*a): return subprocess.run([exe, *a], env=env, capture_output=True, text=True, timeout=60)
orig = open(BIN, 'rb').read()

fresh(); r = run('--update')
chk('same version on GitHub → «Up to date», nothing changed', f'Up to date (v{CUR})' in r.stdout and open(exe, 'rb').read() == orig, (r.stdout, r.stderr))
chk('asks GitHub as «ARION-Pit-Wall/<version>»', STATE.get('ua') == f'ARION-Pit-Wall/{CUR}', STATE.get('ua'))
STATE['tag'] = OLDER; r = run('--update')
chk('an older release is never offered', 'Up to date' in r.stdout, r.stdout)
STATE['tag'] = 'v0.1.99'; STATE['with_sum'] = False; r = run('--update')
chk('a release without a checksum file is never offered', 'Up to date' in r.stdout and open(exe, 'rb').read() == orig, r.stdout)
STATE['with_sum'] = True; STATE['sum'] = '0' * 64; r = run('--update')
chk('a download that doesn\'t match its checksum is refused, nothing changed', r.returncode == 1 and "doesn't match" in r.stderr and open(exe, 'rb').read() == orig and not os.path.exists(exe[:-4] + '.new'), (r.returncode, r.stderr))
STATE['sum'] = hashlib.sha256(NEW).hexdigest(); r = run('--update')
chk('a good newer release: «Updated to v0.1.99», the file is replaced under the same name', 'Updated to v0.1.99' in r.stdout and open(exe, 'rb').read() == NEW, (r.stdout, r.stderr))
chk('the old version is kept as .old until the next start', open(exe[:-4] + '.old', 'rb').read() == orig)
out = subprocess.run([exe, '--x'], capture_output=True, text=True).stdout
chk('the new file is the one that runs', out.strip() == 'new version --x', out)
# next start removes the .old file
fresh(); open(exe[:-4] + '.old', 'wb').write(b'leftover'); STATE['tag'] = 'v' + CUR; run('--update')
chk('the next start removes the leftover .old', not os.path.exists(exe[:-4] + '.old'))
# after an update the new app waits for the old one to free 127.0.0.1:6420 (My dashboard and OBS links keep their address)
s = socket.socket(); s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1); s.bind(('127.0.0.1', 6420)); s.listen(1)
threading.Timer(2.0, s.close).start()
t0 = time.time()
p = subprocess.Popen([exe, '--after-update', '--key', 'arion_tk_' + 'ab' * 32, '--fake', '--url', 'http://127.0.0.1:9', '--apikey', 'x', '--seconds', '6'], env=env, stdout=subprocess.PIPE, text=True)
line = ''
for _ in range(80):
    line = p.stdout.readline()
    if 'My dashboard' in line: break
waited = time.time() - t0; p.wait(timeout=30)
chk('after an update it waits for the old app, then takes 127.0.0.1:6420 again', 'http://127.0.0.1:6420/' in line and waited >= 1.8, (line, round(waited, 1)))
shutil.rmtree(d, ignore_errors=True); srv.shutdown()
print(f'\n{ok} passed, {fail} failed')

"""Stand-in for Supabase during the end-to-end test: the same pitwall_agent() rules as migration 58
(key check, hello/poll/send/bye, answers only to coaches who sent an offer), plus two helper routes
the test page uses in place of the dashboard's table insert/select."""
import json, sys, uuid, threading
from http.server import ThreadingHTTPServer, BaseHTTPRequestHandler
from urllib.parse import urlparse, parse_qs
KEY = 'arion_tk_' + 'ab' * 32
PUB = 'test_pub'
L = threading.Lock(); S = {'session': None, 'sig': [], 'calls': []}
PAGE = open(__file__.replace('mock_db.py', 'viewer.html')).read()
class H(BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'
    def log_message(self, *a): pass
    def send(self, code, obj, ctype='application/json'):
        b = obj.encode() if isinstance(obj, str) else json.dumps(obj).encode()
        self.send_response(code); self.send_header('Content-Type', ctype); self.send_header('Content-Length', str(len(b))); self.end_headers(); self.wfile.write(b)
    def do_GET(self):
        u = urlparse(self.path); q = parse_qs(u.query)
        if u.path == '/page': return self.send(200, PAGE, 'text/html')
        if u.path == '/viewer/answer':
            v = q['viewer'][0]
            with L: a = [s for s in S['sig'] if s['from_agent'] and s['viewer'] == v]
            return self.send(200, a[-1] if a else {})
        if u.path == '/sessions':
            with L:
                rows = [{'id': S['session'], 'user_id': 'u-driver', 'team_id': 1, 'agent_version': '0.1.10', 'started_at': '', 'last_seen': '', **S.get('alert', {})}] if S['session'] else []
                if S.get('ghost'): rows.append({'id': 'ghost', 'user_id': 'u-ghost', 'team_id': 2, 'agent_version': '0.1.10', 'started_at': '', 'last_seen': '', **S['ghost']})
                return self.send(200, rows)
        if u.path in ('/dash', '/mock-supabase.js'):
            import os
            f = os.environ.get('DASH_HTML') if u.path == '/dash' else os.environ.get('DASH_MOCK')
            return self.send(200, open(f).read(), 'text/html' if u.path == '/dash' else 'application/javascript')
        if u.path == '/times':
            with L: return self.send(200, S.get('t',[]))
        if u.path == '/calls':
            with L: return self.send(200, S['calls'])
        self.send(404, {})
    def do_POST(self):
        n = int(self.headers.get('Content-Length', 0)); body = json.loads(self.rfile.read(n) or b'{}')
        u = urlparse(self.path)
        if u.path == '/viewer/offer':
            with L:
                if not S['session']: return self.send(400, {'message': 'no session'})
                S['sig'].append({'id': len(S['sig']) + 1, 'viewer': body['viewer'], 'from_agent': False, 'kind': 'offer', 'body': body['body']})
            return self.send(200, {})
        if u.path == '/test/alert':   # a second car (no app behind it) raises an alert, as migration 59 stores it
            import datetime
            with L: S['ghost'] = {'alert_level': body['level'], 'alert_text': body['text'], 'alert_at': datetime.datetime.now(datetime.timezone.utc).isoformat()}
            return self.send(200, {})
        if u.path == '/viewer/bye':
            with L:
                if S['session']: S['sig'].append({'id': len(S['sig']) + 1, 'viewer': body['viewer'], 'from_agent': False, 'kind': 'bye', 'body': {}})
            return self.send(200, {})
        if u.path != '/rest/v1/rpc/pitwall_agent': return self.send(404, {})
        if self.headers.get('apikey') != PUB: return self.send(401, {'message': 'Invalid API key'})
        k, a, d = body.get('p_key'), body.get('p_action'), body.get('p_data') or {}
        with L:
            S['calls'].append(a); S.setdefault('t',[]).append(round(__import__('time').time(),2))
            if k != KEY: return self.send(400, {'message': 'Unknown key.'})
            if a == 'hello':
                S['session'] = str(uuid.uuid4()); S['sig'] = []
                return self.send(200, {'session': S['session'], 'name': 'DeeDee', 'team': {'id': 1, 'name': 'Arion #82', 'class': 'GT3', 'car': None, 'race': 'Test race'}})
            if not S['session']: return self.send(400, {'message': 'No session. Say hello first.'})
            if a == 'poll':
                aft = int(d.get('after') or 0)
                sig = [{'id': s['id'], 'viewer': s['viewer'], 'kind': s['kind'], 'body': s['body']} for s in S['sig'] if not s['from_agent'] and s['id'] > aft][:50]
                return self.send(200, {'team': {'id': 1, 'name': 'Arion #82', 'class': 'GT3', 'car': None, 'race': 'Test race'}, 'signals': sig})
            if a == 'send':
                if d.get('kind') not in ('answer', 'ice', 'bye'): return self.send(400, {'message': 'Unknown message kind.'})
                if not any(s['viewer'] == d.get('viewer') and s['kind'] == 'offer' and not s['from_agent'] for s in S['sig']): return self.send(400, {'message': 'No such viewer.'})
                S['sig'].append({'id': len(S['sig']) + 1, 'viewer': d['viewer'], 'from_agent': True, 'kind': d['kind'], 'body': d['body']})
                return self.send(200, {'id': len(S['sig'])})
            if a == 'alert':
                import datetime
                if d.get('text') not in ('stopped', 'puncture', 'damage', 'wing off', 'penalty', 'contact', 'track limits', 'pit stop') or d.get('level') not in (1, 2): return self.send(400, {'message': 'Unknown alert.'})
                S['alert'] = {'alert_level': d['level'], 'alert_text': d['text'], 'alert_at': datetime.datetime.now(datetime.timezone.utc).isoformat()}
                return self.send(200, {})
            if a == 'bye':
                S['session'] = None; return self.send(200, {})
            return self.send(400, {'message': 'Unknown action.'})
ThreadingHTTPServer(('127.0.0.1', int(sys.argv[1])), H).serve_forever()

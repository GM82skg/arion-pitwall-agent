"""Stand-in for LMU's local web interface (port 6397), with the shapes TinyPedal reads."""
import json, sys
from http.server import ThreadingHTTPServer, BaseHTTPRequestHandler
def node(sky, t, r): return {"WNV_SKY": {"currentValue": sky}, "WNV_TEMPERATURE": {"currentValue": t}, "WNV_RAIN_CHANCE": {"currentValue": r}}
FC = {"START": node(0, 24, 0), "NODE_25": node(2, 23, 10), "NODE_50": node(3, 21, 30), "NODE_75": node(7, 20, 60), "FINISH": node(8, 19, 80)}
R = {"/rest/sessions/weather": {"PRACTICE": FC, "QUALIFY": FC, "RACE": FC},
     "/rest/garage/UIScreen/RepairAndRefuel": {"wearables": {"body": {"aero": 0.14}, "suspension": [0.06, 0, 0, 0.01], "brakes": [0.032, 0.031, 0.028, 0.028]}, "fuelInfo": {"maxVirtualEnergy": 100.0}},
     "/rest/strategy/pitstop-estimate": {"total": 34.6, "damage": 12.4},
     "/rest/sessions": {}, "/rest/garage/getPlayerGarageData": {}}
class H(BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def do_GET(self):
        b = json.dumps(R.get(self.path, {})).encode()
        self.send_response(200 if self.path in R else 404); self.send_header('Content-Type', 'application/json'); self.send_header('Content-Length', str(len(b))); self.end_headers(); self.wfile.write(b)
ThreadingHTTPServer(('127.0.0.1', int(sys.argv[1]) if len(sys.argv) > 1 else 6397), H).serve_forever()

"""Wait until 127.0.0.1:6420 is free, so a test never talks to the previous test's driver app (it may still be closing)."""
import socket, time
def wait_free(port=6420, secs=30):
    for _ in range(secs * 4):
        s = socket.socket()
        try:
            s.bind(('127.0.0.1', port)); return True
        except OSError:
            time.sleep(0.25)
        finally:
            s.close()
    return False

"""Send iFacialMocap v2 frames using only Python's standard library.
Run the app, select Face, then run: python scripts/mock_tracking.py
"""
import argparse
import math
import socket
import time

parser = argparse.ArgumentParser()
parser.add_argument('--host', default='127.0.0.1')
parser.add_argument('--port', type=int, default=49983)
parser.add_argument('--seconds', type=float, default=60)
args = parser.parse_args()
start = time.monotonic()
with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sock:
    print(f'Sending 60 Hz iFacialMocap to {args.host}:{args.port}; Ctrl+C to stop')
    try:
        while (t := time.monotonic() - start) < args.seconds:
            fields = []
            for eye, offset in [('L', 0), ('R', 0.08)]:
                phase = (t + offset) % 3.4
                blink = math.sin(phase / 0.24 * math.pi) if phase < 0.24 else 0
                gaze = math.sin(t * 0.9) * 65
                # Portrait convention: positive gaze moves both irises screen right.
                outward = gaze if eye == 'L' else -gaze
                fields += [f'eyeBlink_{eye}&{blink*100:.2f}',
                           f'eyeLookOut_{eye}&{max(0, outward):.2f}',
                           f'eyeLookIn_{eye}&{max(0, -outward):.2f}',
                           f'eyeLookUp_{eye}&{max(0, math.sin(t)*35):.2f}',
                           f'eyeLookDown_{eye}&{max(0, -math.sin(t)*35):.2f}',
                           f'browOuterUp_{eye}&{(math.sin(t*0.6)+1)*25:.2f}']
            payload = '|'.join(fields) + f'|=head#0,0,{math.sin(t*0.6)*15:.2f},0,0,0|'
            sock.sendto(payload.encode(), (args.host, args.port))
            time.sleep(1 / 60)
    except KeyboardInterrupt:
        pass

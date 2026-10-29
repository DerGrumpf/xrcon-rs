# Session example: the Session class does no network I/O and has no clock.
# You own the socket and pass the current time in yourself.

import os
import socket
import time

from xrcon import Session

# Read the connection settings from environment variables.
HOST = os.environ["XRCON_HOST"]
PORT = int(os.environ["XRCON_PORT"])
# The password is passed to the session as bytes.
PASSWORD = os.environ["XRCON_PASSWORD"].encode()
# 0 = non-secure, 1 = time-secure, 2 = challenge-secure.
MODE = int(os.environ["XRCON_MODE"])

# How long to keep collecting rcon reply packets after the command is sent.
REPLY_WINDOW = 0.7

# Open and connect our own UDP socket.
sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
sock.connect((HOST, PORT))

# Create the session.
session = Session(PASSWORD, mode=MODE)


def flush():
    # Send every datagram the session has queued.
    while (packet := session.poll_transmit()) is not None:
        sock.send(packet)


def drain_events():
    # Collect all queued events as a list of tuples.
    events = []
    while (event := session.poll_event()) is not None:
        events.append(event)
    return events


def wait_once(timeout):
    # Wait up to `timeout` seconds for one datagram and feed it to the session.
    # On a timeout the session is told the time instead.
    sock.settimeout(max(timeout, 0.001))
    try:
        data = sock.recv(65535)
        session.handle_datagram(data, time.time())
    except socket.timeout:
        session.handle_timeout(time.time())


# --- Ping ---------------------------------------------------------------
# Queue a ping, send it, then wait until the session reports a result.
session.ping("q2", 1.0, time.time())
flush()
done = False
while not done:
    deadline = session.poll_timeout()
    wait_once(deadline - time.time() if deadline else 1.0)
    for event in drain_events():
        if event[0] == "pong":
            # event = ("pong", kind, round trip time in seconds)
            print("ping:", event[2])
            done = True
        elif event[0] == "ping_timeout":
            print("ping: timeout")
            done = True

# --- rcon command ---------------------------------------------------------
# Queue the command. In challenge mode the session first asks the server
# for a challenge, so keep pumping until the reply window has passed.
session.send(b"status", time.time())
flush()
reply = b""
end = None
while True:
    for event in drain_events():
        if event[0] == "response":
            # event = ("response", bytes)
            reply += event[1]
        elif event[0] == "challenge_failed":
            raise SystemExit("server never answered the challenge")

    now = time.time()
    deadline = session.poll_timeout()
    # No pending deadline means the challenge handshake is over (or was
    # never needed), so the reply window starts now.
    if end is None and deadline is None:
        end = now + REPLY_WINDOW
    if end is not None and now >= end:
        break

    # Wait until the next deadline of either the session or the reply window.
    limits = [t for t in (deadline, end) if t is not None]
    wait_once(min(limits) - now)
    # The handshake may have queued a follow-up packet.
    flush()

print("exec:", reply)

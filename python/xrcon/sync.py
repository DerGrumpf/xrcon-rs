"""Blocking client on top of the sans-IO Session."""

import socket
import time
from collections import deque

from ._types import Player
from ._xrcon import Session, colorize
from .aio import ChallengeFailed


class Client:
    """Blocking rcon client. Mode: 0 = nosecure, 1 = time, 2 = challenge."""

    def __init__(self, host, password, mode=1, port=26000):
        self._addr = (host, port)
        self._session = Session(password, mode)
        self._sock = None
        self._events = deque()

    def connect(self):
        family, kind, proto, _, addr = socket.getaddrinfo(
            *self._addr, type=socket.SOCK_DGRAM
        )[0]
        self._sock = socket.socket(family, kind, proto)
        self._sock.connect(addr)

    def close(self):
        if self._sock is not None:
            self._sock.close()
            self._sock = None

    def __enter__(self):
        self.connect()
        return self

    def __exit__(self, *exc):
        self.close()

    # -- plumbing -----------------------------------------------------

    def _pump(self):
        s = self._session
        while (packet := s.poll_transmit()) is not None:
            self._sock.send(packet)
        while (event := s.poll_event()) is not None:
            self._events.append(event)

    def _recv(self, timeout):
        """Waits up to `timeout` seconds (None = forever) for one datagram."""
        self._sock.settimeout(None if timeout is None else max(timeout, 0.001))
        try:
            data = self._sock.recv(65535)
        except TimeoutError:
            self._session.handle_timeout(time.time())
        else:
            self._session.handle_datagram(data, time.time())
        self._pump()

    def _next_event(self):
        while not self._events:
            deadline = self._session.poll_timeout()
            wait = None if deadline is None else deadline - time.time()
            self._recv(wait)
        return self._events.popleft()

    # -- operations ---------------------------------------------------

    def execute(self, command, timeout=0.7, color=False):
        """Send an rcon command. Returns all reply bytes collected until
        `timeout` seconds after the command was sent."""
        self._events.clear()
        self._session.send(command, time.time())
        self._pump()
        chunks = []
        end = None
        while True:
            while self._events:
                event = self._events.popleft()
                if event[0] == "response":
                    chunks.append(event[1])
                elif event[0] == "challenge_failed":
                    raise ChallengeFailed()
            now = time.time()
            deadline = self._session.poll_timeout()
            if end is None and deadline is None:
                end = now + timeout
            if end is not None and now >= end:
                data = b"".join(chunks)
                return colorize(data) if color else data
            targets = [t for t in (deadline, end) if t is not None]
            self._recv(min(targets) - now)

    def ping(self, kind="q2", timeout=1.0):
        """Returns the round trip time in seconds, or None on timeout."""
        self._events.clear()
        self._session.ping(kind, timeout, time.time())
        self._pump()
        while True:
            event = self._next_event()
            if event[0] == "pong":
                return event[2]
            if event[0] == "ping_timeout":
                return None

    def getstatus(self, timeout=1.0):
        """Returns (vars, players), or None on timeout. Raises ValueError
        if the reply cannot be parsed."""
        self._events.clear()
        self._session.getstatus(timeout, time.time())
        self._pump()
        while True:
            event = self._next_event()
            if event[0] == "status":
                return event[1], [Player(*p) for p in event[2]]
            if event[0] == "status_timeout":
                return None
            if event[0] == "status_invalid":
                raise ValueError("invalid status reply")

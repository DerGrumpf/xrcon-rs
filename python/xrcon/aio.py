"""asyncio client on top of the sans-IO Session.

One operation runs at a time: calls are serialized with a lock, because
the Rust Session tracks a single pending operation.
"""

import asyncio
import time
from collections import deque

from ._types import Player
from ._xrcon import Session, colorize


class ChallengeFailed(Exception):
    """The server did not answer the challenge request."""


class _Protocol(asyncio.DatagramProtocol):
    def __init__(self, client):
        self._client = client

    def datagram_received(self, data, addr):
        self._client._on_datagram(data)

    def error_received(self, exc):
        self._client._on_error(exc)


class AsyncClient:
    """Async rcon client. Mode: 0 = nosecure, 1 = time, 2 = challenge."""

    def __init__(self, host, password, mode=1, port=26000):
        self._host = host
        self._port = port
        self._session = Session(password, mode)
        self._transport = None
        self._timer = None
        self._events = deque()
        self._wake = asyncio.Event()
        self._lock = asyncio.Lock()
        self._error = None

    async def connect(self):
        loop = asyncio.get_running_loop()
        self._transport, _ = await loop.create_datagram_endpoint(
            lambda: _Protocol(self), remote_addr=(self._host, self._port)
        )

    def close(self):
        if self._timer is not None:
            self._timer.cancel()
            self._timer = None
        if self._transport is not None:
            self._transport.close()
            self._transport = None

    async def __aenter__(self):
        await self.connect()
        return self

    async def __aexit__(self, *exc):
        self.close()

    # -- plumbing -----------------------------------------------------

    def _pump(self):
        s = self._session
        while (packet := s.poll_transmit()) is not None:
            self._transport.sendto(packet)
        while (event := s.poll_event()) is not None:
            self._events.append(event)
        self._wake.set()
        if self._timer is not None:
            self._timer.cancel()
            self._timer = None
        deadline = s.poll_timeout()
        if deadline is not None:
            loop = asyncio.get_running_loop()
            self._timer = loop.call_later(
                max(0.0, deadline - time.time()), self._on_timer
            )

    def _on_datagram(self, data):
        self._session.handle_datagram(data, time.time())
        self._pump()

    def _on_timer(self):
        self._timer = None
        self._session.handle_timeout(time.time())
        self._pump()

    def _on_error(self, exc):
        self._error = exc
        self._wake.set()

    def _check_error(self):
        if self._error is not None:
            exc, self._error = self._error, None
            raise exc

    async def _next_event(self):
        while True:
            self._check_error()
            if self._events:
                return self._events.popleft()
            self._wake.clear()
            await self._wake.wait()

    # -- operations ---------------------------------------------------

    async def execute(self, command, timeout=0.7, color=False):
        """Send an rcon command. Returns all reply bytes collected until
        `timeout` seconds after the command was sent. setting color applies the colorizer.
        """
        async with self._lock:
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
                self._check_error()
                now = time.time()
                if end is None and self._session.poll_timeout() is None:
                    end = now + timeout
                if end is not None and now >= end:
                    data = b"".join(chunks)
                    return colorize(data) if color else data
                self._wake.clear()
                wait = None if end is None else end - now
                try:
                    await asyncio.wait_for(self._wake.wait(), wait)
                except asyncio.TimeoutError:
                    pass

    async def ping(self, kind="q2", timeout=1.0):
        """Ping the server. Returns the round trip time in seconds, or
        None on timeout. `kind` is 'q2' or 'q3'."""
        async with self._lock:
            self._events.clear()
            self._session.ping(kind, timeout, time.time())
            self._pump()
            while True:
                event = await self._next_event()
                if event[0] == "pong":
                    return event[2]
                if event[0] == "ping_timeout":
                    return None

    async def getstatus(self, timeout=1.0):
        """Query server status. Returns (vars, players), or None on
        timeout. Raises ValueError if the reply cannot be parsed."""
        async with self._lock:
            self._events.clear()
            self._session.getstatus(timeout, time.time())
            self._pump()
            while True:
                event = await self._next_event()
                if event[0] == "status":
                    return event[1], [Player(*p) for p in event[2]]
                if event[0] == "status_timeout":
                    return None
                if event[0] == "status_invalid":
                    raise ValueError("invalid status reply")

# Async Python example: calls are awaited, so other tasks can run
# while this one waits for the server.

import asyncio
import os

from xrcon import AsyncClient

# Read the connection settings from environment variables.
HOST = os.environ["XRCON_HOST"]
PORT = int(os.environ["XRCON_PORT"])
# The password is passed to the client as bytes.
PASSWORD = os.environ["XRCON_PASSWORD"].encode()
# 0 = non-secure, 1 = time-secure, 2 = challenge-secure.
MODE = int(os.environ["XRCON_MODE"])


async def main():
    # `async with` connects on entry and closes the client on exit.
    async with AsyncClient(HOST, PASSWORD, mode=MODE, port=PORT) as client:
        # Round trip time in seconds, or None on timeout.
        print("ping:", await client.ping())

        # Run an rcon command and print the reply.
        print("exec:", await client.execute(b"status"))


# Start the event loop and run main().
asyncio.run(main())

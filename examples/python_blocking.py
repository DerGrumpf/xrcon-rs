# Blocking Python example: each call waits until the server answers.

import os

from xrcon import Client

# Read the connection settings from environment variables.
HOST = os.environ["XRCON_HOST"]
PORT = int(os.environ["XRCON_PORT"])
# The password is passed to the client as bytes.
PASSWORD = os.environ["XRCON_PASSWORD"].encode()
# 0 = non-secure, 1 = time-secure, 2 = challenge-secure.
MODE = int(os.environ["XRCON_MODE"])

# Create the client.
client = Client(HOST, PASSWORD, mode=MODE, port=PORT)

# Round trip time in seconds, or None on timeout.
print("ping:", client.ping())

# Run an rcon command and print the reply.
print("exec:", client.execute(b"status"))

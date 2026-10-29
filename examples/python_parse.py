# Parse example: the parse_* functions and colorize are pure helpers.
# Here they are applied to real packets from the server.

import os
import socket

from xrcon import (
    ParseError,
    Session,
    colorize,
    parse_challenge_response,
    parse_rcon_response,
    parse_server_addr,
    parse_server_vars,
)

# Read the connection settings from environment variables.
HOST = os.environ["XRCON_HOST"]
PORT = int(os.environ["XRCON_PORT"])
# The password is passed to the session as bytes.
PASSWORD = os.environ["XRCON_PASSWORD"].encode()
# 0 = non-secure, 1 = time-secure. Mode 2 is not used in this example.
MODE = int(os.environ["XRCON_MODE"])

# Every packet starts with this 4 byte header.
HEADER = b"\xff\xff\xff\xff"

# "host:port" text -> (host, port).
print("addr:", parse_server_addr(f"{HOST}:{PORT}"))

# Open and connect our own UDP socket.
sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
sock.settimeout(2)
sock.connect((HOST, PORT))

# Challenge: ask the server for one and parse the reply.
sock.send(HEADER + b"getchallenge")
print("challenge:", parse_challenge_response(sock.recv(65535)))

# Server variables: take the vars line of a getstatus reply
# (header + "statusResponse\n" + vars line + "\n" + player lines).
sock.send(HEADER + b"getstatus")
vars_line = sock.recv(65535).split(b"\n")[1]
server_vars = parse_server_vars(vars_line)
print("map:", server_vars[b"mapname"])

# colorize: convert the colour codes of the host name.
print("hostname:", colorize(server_vars[b"hostname"]))

# rcon reply: let a Session build the command packet, send it,
# then parse the first reply packet.
if MODE in (0, 1):
    session = Session(PASSWORD, mode=MODE)
    import time

    session.send(b"status", time.time())
    sock.send(session.poll_transmit())
    print("rcon:", parse_rcon_response(sock.recv(65535)))


# Bad input raises ParseError. This is expected: the packet below is
# deliberately invalid, so the error message is part of the demo.
try:
    parse_rcon_response(b"not a packet")
except ParseError as e:
    print("ParseError:", e)

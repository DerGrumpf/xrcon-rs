from ._types import Player
from ._xrcon import (
    ParseError,
    Session,
    __version__,
    colorize,
    parse_challenge_response,
    parse_rcon_response,
    parse_server_addr,
    parse_server_vars,
    parse_servers_response,
)
from ._xrcon import parse_status_packet as _parse_status_packet
from .aio import AsyncClient, ChallengeFailed
from .sync import Client


def parse_status_packet(packet):
    """Returns (vars, players) with players as Player tuples."""
    vars, players = _parse_status_packet(packet)
    return vars, [Player(*p) for p in players]

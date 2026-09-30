use darkplaces::ServerStatus;

const REPLY: &[u8] = b"host:     Jeff & Julius Resurrection Server\n\
version:  Xonotic Linux dedicated v20140513-1537-g2f25af65~ Jun 21 2026 release GCC \n\
protocol: 3504 (DP7)\n\
map:      ruuttourney1_fix\n\
timing:   36.9% CPU, 0.00% lost, offset avg 1.1ms, max 3.2ms, sdev 0.4ms\n\
players:  6 active (32 max)\n\
\n\
^2IP                                             %pl ping  time   frags  no   name\n\
^382.192.128.164:13321                             0   51  0:37:20    6  #1   ^7PatrokloAkacia\n\
^731.43.214.167:8332                               0  125  0:06:02    4  #2   ^7Max^xA00^7\n\
^392.239.81.78:44095                               0   42  0:04:59    8  #3   ^7eddj\n\
^731.94.2.13:10372                                 0   59  0:04:35    1  #4   ^7^xE03\xe2\x9a\xa0^7\n\
^385.37.63.138:57783                               0   63  0:24:08 -666  #5   ^7Pasta&Pizza\n\
^777.25.47.36:57071                                0   47  2:39:18 -666  #6   ^7^xA0AGrumpf^xF00\xf0\x9f\x94\xa5^7\n";

#[test]
fn parses_real_reply() {
    let s = ServerStatus::parse(REPLY).unwrap();
    assert_eq!(s.host, "Jeff & Julius Resurrection Server");
    assert_eq!(s.protocol_number, 3504);
    assert_eq!(s.protocol_name, "DP7");
    assert_eq!(s.map, "ruuttourney1_fix");
    assert_eq!((s.players_active, s.players_max), (6, 32));
    assert_eq!(s.players.len(), 6);

    let p = &s.players[0];
    assert_eq!(p.ip, "82.192.128.164:13321");
    assert_eq!((p.packetloss, p.ping, p.time_secs), (0, 51, 2240));
    assert_eq!((p.frags, p.slot), (6, 1));
    assert_eq!(p.name, b"PatrokloAkacia");

    assert_eq!(s.players[4].frags, -666);
    assert_eq!(s.players[5].time_secs, 9558);
    assert_eq!(s.players[3].name, b"^xE03\xe2\x9a\xa0^7");
}

#[test]
fn empty_reply_is_error() {
    assert!(ServerStatus::parse(b"").is_err());
}

use darkplaces::parse_maps;

const REPLY: &[u8] = b"^1 289 maps found :\n\
        13castle (Q3BSP46 ) Castle Of Aaargh!\n\
  aim_map-remake (Q3BSP46 ) \n\
aggressorx_wz_04 (Q3BSP46 ) Aggressor\n\
\n";

#[test]
fn parses_real_reply() {
    let m = parse_maps(REPLY);
    assert_eq!(m.len(), 3);
    assert_eq!(m[0].name, "13castle");
    assert_eq!(m[0].format, "Q3BSP46");
    assert_eq!(m[0].title, b"Castle Of Aaargh!");
    assert_eq!(m[1].name, "aim_map-remake");
    assert_eq!(m[1].title, b"");
    assert_eq!(m[2].name, "aggressorx_wz_04");
}

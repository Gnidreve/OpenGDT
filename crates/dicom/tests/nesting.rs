//! Tief verschachtelte Sequenzen aus dem Netzwerk dürfen den Prozess nicht beenden.

use opengdt_dicom::Dataset;

fn nested(levels: usize, implicit: bool) -> Vec<u8> {
    let mut v = Vec::new();
    for _ in 0..levels {
        if implicit {
            v.extend([0x08, 0x00, 0x20, 0x11, 0xFF, 0xFF, 0xFF, 0xFF]); // Element mit undefinierter Länge
        } else {
            v.extend([
                0x08, 0x00, 0x20, 0x11, b'S', b'Q', 0, 0, 0xFF, 0xFF, 0xFF, 0xFF,
            ]);
        }
        v.extend([0xFE, 0xFF, 0x00, 0xE0, 0xFF, 0xFF, 0xFF, 0xFF]); // Item mit undefinierter Länge
    }
    v
}

#[test]
fn deeply_nested_sequences_are_rejected_not_crashing() {
    for implicit in [true, false] {
        // eigener Thread mit der üblichen kleinen Stackgröße, wie bei den Verbindungs-Threads des Servers
        let h = std::thread::Builder::new()
            .stack_size(2 * 1024 * 1024)
            .spawn(move || Dataset::decode(&nested(200_000, implicit), !implicit).is_err())
            .unwrap();
        assert!(
            h.join().expect("Decoder ist abgestürzt"),
            "muss als Fehler abgelehnt werden (implicit={implicit})"
        );
    }
}

#[test]
fn reasonable_nesting_still_works() {
    // 20 Ebenen, sauber geschlossen
    let mut v = Vec::new();
    for _ in 0..20 {
        v.extend([
            0x08, 0x00, 0x20, 0x11, 0xFF, 0xFF, 0xFF, 0xFF, 0xFE, 0xFF, 0x00, 0xE0, 0xFF, 0xFF,
            0xFF, 0xFF,
        ]);
    }
    for _ in 0..20 {
        v.extend([
            0xFE, 0xFF, 0x0D, 0xE0, 0, 0, 0, 0, 0xFE, 0xFF, 0xDD, 0xE0, 0, 0, 0, 0,
        ]);
    }
    assert!(Dataset::decode(&v, false).is_ok());
}

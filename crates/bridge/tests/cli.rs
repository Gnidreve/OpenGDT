//! Kommandozeile: Geheimnisse dürfen nirgends ausgegeben werden.

use std::process::Command;

#[test]
fn check_config_never_prints_the_orthanc_password() {
    let dir = std::env::temp_dir().join(format!("opengdt-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let conf = dir.join("opengdt.conf");
    std::fs::write(&conf, "orthanc_url = http://orthanc:8042\northanc_user = bridge\northanc_password = SuperGeheim123\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_opengdt-bridge"))
        .args(["check-config", conf.to_str().unwrap()])
        .output()
        .unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.status.success(), "{text}");
    assert!(
        text.contains("orthanc_user"),
        "Ausgabe enthält die Konfiguration"
    );
    assert!(
        !text.contains("SuperGeheim123"),
        "Passwort steht in der Ausgabe:\n{text}"
    );
    // auch über die Umgebung gesetzte Passwörter
    let out = Command::new(env!("CARGO_BIN_EXE_opengdt-bridge"))
        .args(["check-config", conf.to_str().unwrap()])
        .env("OPENGDT_ORTHANC_PASSWORD", "AusDerUmgebung456")
        .output()
        .unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!text.contains("AusDerUmgebung456"), "{text}");
    let _ = std::fs::remove_dir_all(dir);
}

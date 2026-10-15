# Konzept: In-house DICOM-GDT-Bridge für Ultraschallsysteme

> Strategisches und technisches Konzeptpapier. Es beschreibt das *Warum* und die
> grobe Richtung. Wie der Code tatsächlich aufgebaut ist, steht in den READMEs der
> jeweiligen Ordner unter [`crates/`](../crates/README.md).
>
> **Quellenlage:** Die ursprüngliche Fassung dieses Papiers stützte sich pauschal auf
> „sonogdt“, „sonofair“, „padsy.info“ und „qms-standards.de“. Einzelbelege waren nicht
> nachvollziehbar. Alle Zahlen und regulatorischen Aussagen unten sind deshalb als
> **ungeprüfte Annahmen** zu lesen, bis sie belegt sind. Offene Punkte stehen in
> [Abschnitt 6](#6-offene-punkte-und-risiken).

## 1. Ausgangslage

Moderne Ultraschallgeräte (z. B. Mindray, Alpinion) sprechen den Bildstandard **DICOM**.
Deutsche Praxisverwaltungssysteme (PVS) sprechen traditionell das schlanke, textbasierte
**GDT** (Gerätedatentransfer). Zwischen beiden braucht es einen „Dolmetscher“.

Am Markt wird diese Anbindung typischerweise als kostenpflichtige Erstanbindung
verkauft (Größenordnung 2.000–3.000 €, *ungeprüfte Angabe*), teils ergänzt um
Schnittstellenlizenzen des PVS-Herstellers. Für ein Unternehmen, das Ultraschallsysteme
vertreibt, ist eine eigene Bridge deshalb interessant: Sie schließt die Wertschöpfungskette,
sichert Marge und liefert Kunden ein Gesamtsystem aus einer Hand.

## 2. Architektur

Die Bridge ist ein kleiner Hintergrunddienst (z. B. auf einem Praxis-Server oder
Raspberry Pi) mit vier Aufgaben:

```
 Hinweg
 PVS ──(GDT-Datei)──▶ ① GDT-Reader ──▶ ② Worklist-Server ──(DICOM MWL)──▶ Ultraschallgerät

 Rückweg
 Ultraschallgerät ──(DICOM C-STORE)──▶ ③ Storage-Server ──▶ ④ Konverter ──▶ ⑤ GDT-Writer ──(GDT-Datei)──▶ PVS
                                                              │
                                                              └──▶ Bilddateien im Netzwerkordner
```

| Komponente | Aufgabe | Code |
|---|---|---|
| ① GDT-Reader | liest die Untersuchungsanforderung des PVS (Patientendaten) | [`crates/gdt`](../crates/gdt/README.md) |
| ② Worklist-Server | stellt die Patientendaten dem Gerät als DICOM Modality Worklist bereit (kein Abtippen am Gerät) | [`crates/worklist`](../crates/worklist/README.md), [`crates/dicom`](../crates/dicom/README.md) |
| ③ Storage-Server | nimmt Bilder/Clips per C-STORE entgegen und archiviert die Originale | [`crates/store`](../crates/store/README.md), [`crates/dicom`](../crates/dicom/README.md) |
| ④ Konverter | macht aus DICOM-Pixeldaten Dateien, die ein PVS öffnen kann (JPEG, MP4, PNG, PDF) | [`crates/convert`](../crates/convert/README.md) |
| ⑤ GDT-Writer | schreibt die Rückmeldung mit Verweisen auf die Bilddateien in den Importordner des PVS | [`crates/gdt`](../crates/gdt/README.md) |
| Orchestrierung | Konfiguration, Überwachung, Ablaufsteuerung | [`crates/bridge`](../crates/bridge/README.md) |

## 3. Aufwand und Komplexität

* **Prototyp:** Ein funktionsfähiger Kern ist in Tagen bis Wochen machbar (dieses Repository
  ist der Beleg: Hin- und Rückweg laufen Ende-zu-Ende, siehe [Status](../README.md#status)).
* **PVS-Vielfalt:** GDT ist zeilenbasiert und starr, die Unterschiede zwischen PVS liegen
  in Details (Zeichensatz, Pfade, Dateinamen, Satzarten). Sie lassen sich über
  Konfiguration abfangen.
* **Das eigentlich Schwierige** liegt im Betrieb: instabile Praxisnetzwerke, restriktive
  Windows-Firewalls, Geräte mit eigenwilligem DICOM-Verhalten, Patientenverwechslung
  bei manuell am Gerät angelegten Patienten.
* **Gerätevarianz:** Die frühere Annahme, die Beschränkung auf Mindray und Alpinion
  eliminiere die DICOM-Varianz, ist **unbelegt**. Die tatsächlichen Transfer Syntaxes,
  Tags und Cine-Formate dieser Geräte müssen an echten Geräten geprüft werden.

## 4. Lizenzfreies Testen

GDT ist eine reine Dateischnittstelle über Verzeichnisse. Das PVS lässt sich daher
vollständig simulieren, ohne Lizenzen zu erwerben:

1. **Spezifikation:** Die Feldkennungen sind dokumentiert (Qualitätsring Medizinische
   Software – QMS; PVS-Hersteller veröffentlichen eigene GDT-Beschreibungen).
2. **Anforderung simulieren:** Eine GDT-Datei mit Patientendaten von Hand in den
   Importordner legen.
3. **Rückmeldung prüfen:** Die erzeugte GDT-Datei mit der Spezifikation vergleichen
   (Zeilenlängen, Feldkennungen, Satzlänge).
4. **DICOM-Seite:** Gegen unabhängige Implementierungen testen (im Projekt wurde
   `pynetdicom` als Gegenstelle verwendet; es gehört **nicht** zum Repository).
5. **Rauchtest:** Ein echter Test an einem Demosystem bzw. in einer kooperierenden Praxis
   bleibt unverzichtbar und kostet nichts außer Zeit.

Siehe auch [`tests/interop`](../tests/interop/README.md) für den Interop-Test und `cargo test --workspace` für die übrigen.

## 5. Entscheidungen

| Entscheidung | Begründung |
|---|---|
| Rust, nur `std` | Ein statisches Binary ohne Laufzeit; keine Lieferketten-Risiken; passt zu einem Gerät in einer Arztpraxis. |
| Originale zuerst archivieren | Die DICOM-Originale sind die Quelle der Wahrheit. Konvertierung ist wiederholbar. |
| Nichts neu kodieren, was schon JPEG/MP4 ist | Ultraschall liefert häufig JPEG-Baseline oder MPEG-4 in den DICOM-Fragmenten; Auspacken ist verlustfrei und billig. |
| Dateisystem statt Datenbank | GDT ist dateibasiert; Zustand liegt in Verzeichnissen und ist mit Bordmitteln inspizierbar. |

## 6. Offene Punkte und Risiken

* **MDR (EU 2017/745):** Die Einstufung der Software als Medizinprodukt ist **rechtlich zu
  klären**, bevor sie ausgeliefert wird. Die frühere Aussage, der Zertifizierungsaufwand
  lasse sich „oft schlank halten“, ist ungeprüft. Software, die Patientendaten mit Bildern
  verknüpft, birgt ein Verwechslungsrisiko und ist kein Randthema.
* **Datenschutz (DSGVO, Art. 9):** Bilder und Patientendaten liegen in Netzwerkordnern
  und im Archiv. Zugriffsschutz, Aufbewahrung und Löschung sind zu definieren.
  Der DICOM-Port ist unverschlüsselt (kein TLS in dieser Version).
* **Patientenzuordnung:** Wird am Gerät ein Patient von Hand angelegt, stimmt die
  PatientID nicht mit dem PVS überein; die GDT-Rückmeldung kann dann nicht zugeordnet werden.
* **GDT-Details:** Satzarten und einige Feldkennungen (Dateiverweise) sind in diesem
  Repository noch nicht gegen die offizielle Spezifikation geprüft, siehe
  [`crates/gdt/CLAUDE.md`](../crates/gdt/CLAUDE.md). Die frühere Angabe, eine
  Anforderungsdatei trage Satzart „6302“, ist vermutlich falsch (Anforderung vermutlich
  „6301“) und wurde deshalb nicht übernommen.

# SEPA-GiroCode-Generator · Linux (Rust)

Lokale Linux-Desktopanwendung für SEPA-Überweisungen in Euro. Eine PDF-, JPG- oder PNG-Rechnung öffnen, die erkannten Felder mit dem Original vergleichen und den GiroCode unter den Eingabefeldern anzeigen. Änderungen an Zahlungsdaten entfernen die alte Vorschau.

Die Anwendung ist in Rust geschrieben. Qt 6 Widgets stellt die Plasma-Oberfläche bereit, die Poppler-Bibliothek liest PDFs und die Tesseract-Bibliothek übernimmt die Texterkennung, und die Rust-Bibliothek qrcode erzeugt den GiroCode. Die Verarbeitung läuft lokal.

Markiere Wörter oder mehrere Zeilen direkt auf der Rechnung mit gedrückter linker Maustaste. Mit **Strg+C** oder **Auswahl kopieren** gelangt der markierte Text in die Zwischenablage. Bei Fotos und Scans wird der erkannte OCR-Text kopiert.

## Bauen und starten

Benötigt werden eine aktuelle stabile Rust-Toolchain mit Cargo, ein C++-Compiler, CMake, Ninja und die Entwicklungspakete für Qt 6 Widgets und Qt Concurrent. Für den nativen Build sind die Entwicklungspakete von Poppler-GLib, Cairo und Tesseract erforderlich (auf Ubuntu beispielsweise `libpoppler-glib-dev` und `libtesseract-dev`). Die Flatpak-Fassung bringt die Bibliotheken und beide OCR-Sprachmodelle mit. Beim nativen Start lässt sich der Modellordner mit `SEPA_GIROCODE_TESSDATA` festlegen. Der Dateidialog nutzt im Flatpak das Desktop-Portal. Für Drag-and-drop aus Dolphin hat das Flatpak Lesezugriff auf Dokumente und Downloads. Dateien aus anderen Ordnern können über den Dateidialog geöffnet werden.

```sh
cargo test
cmake -S . -B build -G Ninja -DCMAKE_BUILD_TYPE=Release
cmake --build build
ctest --test-dir build --output-on-failure
./build/rechnung-zu-girocode
```

Die Anwendung liest beschriftete Zahlungsfelder und einfache zweispaltige Briefköpfe. Soweit Angaben fehlen, können sie manuell ergänzt werden. Die IBAN-Prüfsumme bestätigt ausschließlich die formale Gültigkeit. Empfänger, Betrag und Verwendungszweck vor dem Scannen prüfen.

Desktop-Praxistests unter Plasma und GNOME sowie ein Test mit einer Banking-App sind offen. Die kombinierte Anwendung wird unter der GNU GPL v3 veröffentlicht. Frühere MIT-Hinweise sind in `LICENSE-MIT` dokumentiert. Tesseract und die mitgelieferten Sprachmodelle stehen unter Apache 2.0.

## Lizenzen und Drittanbieter

Unter **Hilfe → Open-Source-Lizenzen** stehen eine Übersicht und die im Anwendungspaket mitgelieferten Lizenztexte. Beim Bau wird aus `cargo metadata` ein Verzeichnis der aufgelösten Rust-Pakete mit Lizenzangaben und vorhandenen Lizenzdateien erzeugt. Es kann Bau- und optionale Abhängigkeiten enthalten; fehlende Lizenztexte werden ausdrücklich gekennzeichnet. Die separat installierte KDE-Flatpak-Laufzeit hat eigene Lizenz- und Drittanbieterhinweise und ist in diesem Verzeichnis nicht vollständig inventarisiert. Die Übersicht ersetzt keine Prüfung der Lizenzbedingungen einer konkreten Veröffentlichung.

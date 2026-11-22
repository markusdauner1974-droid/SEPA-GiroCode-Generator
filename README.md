# Rechnung zu GiroCode · Linux (Rust)

Entwicklungszweig für die lokale Linux-Desktopanwendung. Eine PDF-, JPG- oder PNG-Rechnung öffnen, die erkannten Felder mit dem Original vergleichen und den GiroCode unter den Eingabefeldern anzeigen. Änderungen an Zahlungsdaten entfernen die alte Vorschau.

Die Anwendung ist in Rust geschrieben. eframe/egui zeichnet die Oberfläche, Poppler liest PDFs, Tesseract übernimmt die Texterkennung, und die Rust-Bibliothek qrcode erzeugt den GiroCode. Die Verarbeitung läuft lokal. Temporäre Seiten für die Texterkennung liegen für die Dauer des Einlesens in einem temporären Verzeichnis.

## Bauen und starten

Benötigt werden eine aktuelle stabile Rust-Toolchain mit Cargo, ein C-Linker sowie die Linux-Entwicklungspakete für den Grafikstack (Wayland/X11 und EGL). Zur Laufzeit müssen pdftotext, pdftoppm und tesseract im PATH liegen. Für OCR wird mindestens eines der Tesseract-Sprachpakete Deutsch oder Englisch benötigt. Der Dateidialog nutzt das Desktop-Portal.

```sh
cargo test
cargo run --release
```

Die Anwendung liest beschriftete Zahlungsfelder und einfache zweispaltige Briefköpfe. Soweit Angaben fehlen, können sie manuell ergänzt werden. Die IBAN-Prüfsumme bestätigt ausschließlich die formale Gültigkeit. Empfänger, Betrag und Verwendungszweck vor dem Scannen prüfen.

Desktop-Praxistests unter Plasma und GNOME sowie ein Test mit einer Banking-App sind offen. Lizenz: MIT.

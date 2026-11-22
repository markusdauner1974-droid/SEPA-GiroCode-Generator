# Rechnung zu GiroCode

Lokales Desktop-Programm für SEPA-Überweisungen in Euro. Öffne eine PDF-Rechnung, ein JPG oder ein PNG. Vergleiche die Vorschläge mit dem Beleg links im Fenster. Ergänze fehlende Felder und klicke auf **GiroCode erstellen**. Der Code erscheint in einem eigenen Fenster und lässt sich mit einer kompatiblen Banking-App scannen.

## Auf Linux, Windows oder macOS starten

1. Installiere [Miniforge](https://github.com/conda-forge/miniforge#install) für dein Betriebssystem.
2. Lade den Projektordner herunter und entpacke ihn.
3. Öffne im Projektordner ein Terminal. Unter Windows nutze den **Miniforge Prompt** und wechsle mit `cd` in den entpackten Ordner.
4. Führe die folgenden Befehle nacheinander aus.

```sh
conda env create -f environment.yml
conda activate girocode
python girocode.py
```

Die Einrichtung benötigt Internetzugang. Für spätere Starts genügen die letzten beiden Befehle im Projektordner. Die Anwendung verarbeitet Rechnungen offline und speichert weder erkannte Zahlungsdaten noch QR-Bilder.

## Erkennung und Grenzen des Prototyps

Poppler liest den PDF-Text und rendert die Seiten für die Vorschau. Tesseract übernimmt Fotos und PDF-Seiten mit weniger als 24 sichtbaren Textzeichen. Deutsch und Englisch werden je nach installierten Sprachdaten verwendet. Mit `tesseract --list-langs` lässt sich die Installation prüfen. Gemischte Seiten mit Text und eingebettetem Foto können manuelle Ergänzungen erfordern.

Die Vorschläge verwenden deutschsprachige Feldbezeichnungen. Fehlt die Empfängerbeschriftung, prüft das Programm den Briefkopf auf einen Firmennamen in der Spalte der Umsatzsteuer-ID. Bleibt die Zuordnung unklar, trägst du den Empfänger selbst ein. Bei mehreren erkannten IBANs ist das Auswahlfeld zunächst leer. Prüfe bei Gutschriften, Lastschriftankündigungen und bereits bezahlten Rechnungen, ob überhaupt eine Überweisung ansteht. Die Anwendung prüft die IBAN-Prüfsumme und den Euro-Betrag. Einen Abgleich mit Bankkonten oder dem Zahlungsstatus führt sie nicht durch.

Das QR-Fenster zeigt die bestätigten Angaben als EPC-QR für eine SEPA-Überweisung. Als Verwendungszweck unterstützt der Prototyp freien Text. Strukturierte Gläubigerreferenzen (RF) und Schweizer QR-Rechnungen gehören nicht zum Funktionsumfang.

## Tests

```sh
python -m unittest -v
```

Die Tests erzeugen ausschließlich synthetische Beispieldokumente. Für die unabhängige QR-Rücklesung wird zusätzlich `zbarimg` verwendet, sofern installiert.

Lizenz des Projektcodes: MIT. Abhängigkeiten behalten ihre jeweiligen Lizenzen.

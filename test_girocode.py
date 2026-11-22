import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

from reportlab.pdfgen.canvas import Canvas

from beleg import read_document
from zahlung import payload, qr_image, suggestions


def test_account(bban):
    check = 98 - int(bban + '131400') % 97
    return f'DE{check:02d}{bban}'


ACCOUNT = test_account('000000000000000001')
FIELDS = ['Beispielwerk GmbH', ACCOUNT, '', '127,50', 'Rechnung TEST-17']


class PaymentTests(unittest.TestCase):
    def test_pdf_and_photo(self):
        with tempfile.TemporaryDirectory() as folder:
            pdf = Path(folder, 'beispiel.pdf')
            doc = Canvas(str(pdf))
            doc.setFont('Helvetica', 16)
            for i, line in enumerate(['Empfänger: Beispielwerk GmbH', 'IBAN: ' + ACCOUNT,
                                      'Rechnungsbetrag: 127,50 EUR', 'Rechnungsnummer: TEST-17']):
                doc.drawString(40, 750 - i * 42, line)
            doc.save()
            pages, text = read_document(pdf)
            self.assertEqual(suggestions(text)[0], FIELDS)
            jpg = pdf.with_suffix('.jpg')
            pages[0].save(jpg, quality=93)
            _, photo_text = read_document(jpg)
            self.assertEqual(suggestions(photo_text)[0], FIELDS)

    def test_sender_column_with_wrapped_legal_form(self):
        text = ("Kundenfirma GmbH         Nordlicht Beratung\n"
                "Einkauf                  GmbH & Co. KG\n"
                "Musterweg 12             USt-ID: TEST\n"
                "RECHNUNG\nKomplementärin: Andere GmbH")
        self.assertEqual(suggestions(text)[0][0], "Nordlicht Beratung GmbH & Co. KG")
        self.assertEqual(suggestions(text + "\nKontoinhaber: Zahlstelle AG")[0][0], "Zahlstelle AG")
        self.assertEqual(suggestions(text.replace("USt-ID: TEST", "Telefon: 123"))[0][0], "")

    def test_recipient_from_two_column_image(self):
        with tempfile.TemporaryDirectory() as folder:
            pdf = Path(folder, 'briefkopf.pdf')
            doc = Canvas(str(pdf))
            doc.setFont('Helvetica', 14)
            for i, (left, right) in enumerate([
                ('Kundenfirma GmbH', 'Nordlicht Beratung'),
                ('Einkauf', 'GmbH & Co. KG'),
                ('Am Platz 12', 'Marktweg 18'),
                ('Kundennummer: 44', 'USt-ID: DE123456789'),
            ]):
                doc.drawString(35, 740 - i * 24, left)
                doc.drawString(315, 740 - i * 24, right)
            doc.drawString(35, 560, 'Rechnung')
            doc.save()
            pages, _ = read_document(pdf)
            for extension in ('jpg', 'png'):
                with self.subTest(extension=extension):
                    path = pdf.with_suffix('.' + extension)
                    pages[0].save(path)
                    _, text = read_document(path)
                    self.assertEqual(suggestions(text)[0][0], 'Nordlicht Beratung GmbH & Co. KG', text)

    def test_multiple_accounts(self):
        second = test_account('000000000000000002')
        fields, accounts = suggestions(ACCOUNT + '\nIBAN: ' + second)
        self.assertEqual(accounts, [ACCOUNT, second])
        self.assertEqual(fields[1], '')

    def test_invalid_data(self):
        for index, value in [(0, ''), (1, 'DE00' + ACCOUNT[4:]), (3, '-1,00'), (3, '0,00'),
                             (3, '1.234'), (4, 'A\nB')]:
            fields = FIELDS.copy()
            fields[index] = value
            with self.subTest(value=value), self.assertRaises(ValueError):
                payload(*fields)

    @unittest.skipUnless(shutil.which('zbarimg'), 'zbarimg fehlt')
    def test_qr_roundtrip(self):
        data = payload(*FIELDS)
        self.assertEqual(data.decode().split('\n'), ['BCD', '002', '1', 'SCT', '', FIELDS[0],
                                                      ACCOUNT, 'EUR127.50', '', '', FIELDS[4]])
        with tempfile.TemporaryDirectory() as folder:
            qr = Path(folder, 'test.png')
            qr_image(data).save(qr)
            decoded = subprocess.run(['zbarimg', '--quiet', '--raw', str(qr)], check=True,
                                     capture_output=True).stdout.rstrip(b'\r\n')
            self.assertEqual(decoded, data)

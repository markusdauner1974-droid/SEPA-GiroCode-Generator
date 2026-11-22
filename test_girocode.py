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

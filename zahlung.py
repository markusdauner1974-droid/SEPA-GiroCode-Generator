import re
from decimal import Decimal

from PIL import Image
from reportlab.graphics.barcode import qrencoder

# SEPA IBAN lengths by country prefix.
LENGTH = dict(DE=22, AT=20, BE=16, BG=22, HR=21, CY=28, CZ=24, DK=18,
              EE=20, FI=18, FR=27, GR=27, HU=28, IE=22, IT=27, LV=21,
              LT=20, LU=20, MT=31, NL=18, PL=28, PT=25, RO=24, SK=24,
              SI=19, ES=24, SE=24, IS=26, LI=21, NO=15, CH=21, GB=22,
              AD=24, MC=27, SM=27, VA=22, GI=23, AL=28, ME=22, MD=24,
              MK=19, RS=22)
EEA = set('AT BE BG HR CY CZ DK EE FI FR DE GR HU IE IT LV LT LU MT NL PL PT RO SK SI ES SE IS LI NO'.split())


def valid_iban(value):
    if not re.fullmatch(r'[A-Z]{2}[0-9]{2}[A-Z0-9]+', value):
        return False
    if len(value) != LENGTH.get(value[:2]):
        return False
    digits = ''.join(str(int(c, 36)) for c in value[4:] + value[:4])
    return int(digits) % 97 == 1


def issuer_from_header(text):
    """Identify a company in the issuer column of a layout-preserving PDF."""
    header = re.split(r'^\s*Rechnung(?:snummer)?\s*[:\n]', text, maxsplit=1, flags=re.I | re.M)[0]
    rows = []
    for line in header.expandtabs().splitlines()[:35]:
        cells = list(re.finditer(r'\S+(?: [^\s]+)*', line))
        rows.append(cells)
    # A supplier tax label anchors the column; a customer address alone
    # is insufficient evidence for choosing a payment recipient.
    anchors = {c.start() for row in rows for c in row
               if re.match(r'USt[. -]*(?:Id|Ident)|Umsatzsteuer-ID', c[0], re.I)}
    suffix = re.compile(r'(?:GmbH(?: & Co\. KG)?|AG|KG|OHG|GbR|e\.V\.)$', re.I)
    names = set()
    for col in anchors:
        previous = ''
        for row in rows:
            aligned = [c[0] for c in row if abs(c.start() - col) <= 2]
            cell = aligned[0] if len(aligned) == 1 else ''
            if re.match(r'USt[. -]*(?:Id|Ident)|Umsatzsteuer-ID', cell, re.I):
                break
            if suffix.search(cell):
                name = previous + ' ' + cell if suffix.fullmatch(cell) else cell
                if (not suffix.fullmatch(name.strip()) and len(name) <= 70
                        and re.fullmatch(r"[^\W\d_][\w &.,'’()/+-]*", name)):
                    names.add(name)
            previous = cell
    return names.pop() if len(names) == 1 else ''


def suggestions(text):
    accounts = []
    for m in re.finditer(r'\b([A-Z]{2})(?=[0-9]{2})', text):
        size = LENGTH.get(m[1])
        if size:
            tail = text[m.start():]
            pattern = rf'(?:[A-Z0-9][ \t]*){{{size-1}}}[A-Z0-9](?![A-Z0-9])'
            found = re.match(pattern, tail)
            if found:
                candidate = ''.join(found[0].split())
                if valid_iban(candidate) and candidate not in accounts:
                    accounts.append(candidate)

    def single(pattern):
        values = {m.strip() for m in re.findall(pattern, text, re.I | re.M)}
        return values.pop() if len(values) == 1 else ''

    labels = re.findall(r'(?:^|[ \t]{2,})(?:Kontoinhaber(?:in)?|Zahlungsempfänger|Empfänger)[ \t]*:[ \t]*([^\n]+)', text, re.I | re.M)
    names = {re.split(r'\s{2,}|\s+(?:IBAN|BIC)\s*:', label.strip(), maxsplit=1, flags=re.I)[0]
             for label in labels}
    name = names.pop() if len(names) == 1 else ''
    if not labels:
        name = issuer_from_header(text)
    amount = single(r'(?:zu zahlender Betrag|Zahlbetrag|offener Betrag|Gesamtbetrag|Rechnungsbetrag)[ \t:]*(?:EUR|€)?[ \t]*(\d+(?:\.\d{3})*,\d{2}|\d+\.\d{2})(?!\d)')
    if re.search(r'\b(?:CHF|USD|GBP|SEK|NOK|DKK|PLN|CZK)\b|[$£]', text):
        amount = ''
    purpose = single(r'^(?:Verwendungszweck|Zahlungsgrund)[ \t]*:[ \t]*([^\n]+)')
    number = single(r'\b(?:Rechnungsnummer|Rechnungs[- ]?Nr\.?)[ \t:#]*([\w/.-]*\d[\w/.-]*)')
    bic = single(r'\bBIC[ \t:]*([A-Z]{6}[A-Z0-9]{2}(?:[A-Z0-9]{3})?)\b')
    return [name, accounts[0] if len(accounts) == 1 else '', bic if len(accounts) == 1 else '',
            amount, purpose or ('Rechnung ' + number if number else '')], accounts


def payload(name, iban, bic, amount, purpose):
    name, purpose = name.strip(), purpose.strip()
    if not name or len(name) > 70 or len(purpose) > 140:
        raise ValueError('Empfänger (1–70 Zeichen) und Verwendungszweck (max. 140) prüfen.')
    if any(not c.isprintable() for c in name + purpose):
        raise ValueError('Bitte Steuerzeichen aus den Textfeldern entfernen.')
    iban = ''.join(iban.split()).upper()
    if not valid_iban(iban):
        raise ValueError('IBAN prüfen. Länge, Land oder Prüfsumme ist ungültig.')
    bic = bic.strip().upper()
    if (bic or iban[:2] not in EEA) and not re.fullmatch('[A-Z]{6}[A-Z0-9]{2}([A-Z0-9]{3})?', bic):
        raise ValueError('Bitte eine gültige BIC der Empfängerbank eingeben.')
    amount = amount.strip()
    if re.fullmatch(r'\d{1,3}(?:\.\d{3})+,\d{2}', amount):
        amount = amount.replace('.', '')
    if not re.fullmatch(r'[0-9]{1,9}([.,][0-9]{1,2})?', amount):
        raise ValueError('Euro-Betrag prüfen, zum Beispiel 127,50.')
    euros = Decimal(amount.replace(',', '.'))
    if euros <= 0:
        raise ValueError('Der Betrag muss größer als null sein.')
    data = '\n'.join(['BCD', '002', '1', 'SCT', bic, name, iban, f'EUR{euros:.2f}', '', '', purpose]).rstrip('\n').encode()
    if len(data) > 331:
        raise ValueError('Die Zahlungsdaten sind zu lang. Bitte Verwendungszweck kürzen.')
    return data


def qr_image(data):
    code = qrencoder.QRCode(None, qrencoder.QRErrorCorrectLevel.M)
    code.addData(qrencoder.QR8bitByte(data))
    code.make()
    size = code.getModuleCount()
    image = Image.new('L', (size + 8, size + 8), 255)
    for y in range(size):
        for x in range(size):
            if code.isDark(y, x):
                image.putpixel((x + 4, y + 4), 0)
    scale = max(4, 420 // image.width)
    return image.resize((image.width * scale, image.height * scale), Image.Resampling.NEAREST)

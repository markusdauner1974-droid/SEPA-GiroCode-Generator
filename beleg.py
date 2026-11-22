import csv
import io
import subprocess
from pathlib import Path
from statistics import median

from PIL import Image, ImageOps


def run(*args, data=None):
    try:
        return subprocess.run(args, input=data, capture_output=True, check=True, timeout=150).stdout
    except FileNotFoundError as exc:
        raise ValueError(f'{args[0]} fehlt. Bitte die Umgebung gemäß README installieren.') from exc
    except subprocess.CalledProcessError as exc:
        raise ValueError(f'{args[0]} konnte den Beleg nicht lesen. Format, Passwortschutz und Installation prüfen.') from exc


def layout_text(tsv):
    """Reconstruct rows and column starts from Tesseract word boxes."""
    words = []
    for box in csv.DictReader(io.StringIO(tsv), delimiter='\t', quoting=csv.QUOTE_NONE):
        word = (box.get('text') or '').strip()
        if box['level'] == '5' and word:
            x, y, width, height = (int(box[k]) for k in ('left', 'top', 'width', 'height'))
            words.append((y + height / 2, x, width, height, word))
    if not words:
        return ''
    height = median(w[3] for w in words)
    unit = max(1, median(w[2] / len(w[4]) for w in words) * 0.8)
    rows = []
    for word in sorted(words):
        if not rows or word[0] - rows[-1][0][0] > height * 0.65:
            rows.append([])
        rows[-1].append(word)
    lines = []
    for row in rows:
        line = ''
        end = None
        for _, x, width, _, word in sorted(row, key=lambda w: w[1]):
            if end is None or x - end > 3 * unit:
                padding = max(0 if end is None else 2, round(x / unit) - len(line))
            else:
                padding = 1
            line += ' ' * padding + word
            end = x + width
        lines.append(line)
    return '\n'.join(lines)


def ocr(image):
    buffer = io.BytesIO()
    image.save(buffer, 'PNG')
    languages = run('tesseract', '--list-langs').decode().splitlines()
    lang = '+'.join(s for s in ('deu', 'eng') if s in languages)
    if not lang:
        raise ValueError('Tesseract benötigt deutsche oder englische Sprachdaten.')
    tsv = run('tesseract', 'stdin', 'stdout', '-l', lang, 'tsv', data=buffer.getvalue())
    return layout_text(tsv.decode('utf-8'))


def read_document(file):
    path = Path(file).resolve()
    pages, texts = [], []
    if path.suffix.lower() == '.pdf':
        raw = run('pdftotext', '-layout', '-enc', 'UTF-8', str(path), '-').decode()
        texts = raw.split('\f')
        if texts[-1] == '':
            texts.pop()
        for index, text in enumerate(texts, 1):
            png = run('pdftoppm', '-f', str(index), '-l', str(index), '-singlefile',
                      '-scale-to', '2100', '-png', str(path))
            with Image.open(io.BytesIO(png)) as src:
                image = src.convert('RGB')
            if len(text.strip()) < 24:
                texts[index - 1] = ocr(image)
            image.thumbnail((1500, 1800))
            pages.append(image)
    else:
        with Image.open(path) as src:
            image = ImageOps.exif_transpose(src).convert('RGB')
        image.thumbnail((3200, 3200))
        texts = [ocr(image)]
        pages = [image]
    if not pages:
        raise ValueError('Der Beleg enthält keine lesbaren Seiten.')
    return pages, '\n'.join(texts)

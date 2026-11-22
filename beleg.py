import io
import subprocess
from pathlib import Path

from PIL import Image, ImageOps


def run(*args, data=None):
    try:
        return subprocess.run(args, input=data, capture_output=True, check=True, timeout=150).stdout
    except FileNotFoundError as exc:
        raise ValueError(f'{args[0]} fehlt. Bitte die Umgebung gemäß README installieren.') from exc
    except subprocess.CalledProcessError as exc:
        raise ValueError(f'{args[0]} konnte den Beleg nicht lesen. Format, Passwortschutz und Installation prüfen.') from exc


def ocr(image):
    buffer = io.BytesIO()
    image.save(buffer, 'PNG')
    languages = run('tesseract', '--list-langs').decode().splitlines()
    lang = '+'.join(s for s in ('deu', 'eng') if s in languages)
    if not lang:
        raise ValueError('Tesseract benötigt deutsche oder englische Sprachdaten.')
    return run('tesseract', 'stdin', 'stdout', '-l', lang, data=buffer.getvalue()).decode('utf-8')


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

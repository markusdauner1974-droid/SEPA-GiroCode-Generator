use std::{ffi::OsStr, path::Path};
use image::{DynamicImage, GenericImageView};
use glib::translate::ToGlibPtr;
use crate::selection::{self, Word};

fn layout_text(tsv: &str) -> Result<String, String> {
    let mut words = Vec::new();
    let mut heights = Vec::new();
    let mut units = Vec::new();
    for row in tsv.lines().skip(1) {
        let f: Vec<_> = row.splitn(12,'\t').collect();
        if f.len()!=12 || f[0]!="5" || f[11].trim().is_empty() { continue; }
        let coords: Result<Vec<i32>,_> = f[6..10].iter().map(|s| s.parse()).collect();
        let v = coords.map_err(|_| "Ungültige OCR-Koordinaten.")?;
        words.push((v[1]+v[3]/2,v[0],v[2],f[11].trim()));
        heights.push(v[3]);
        units.push(v[2]/f[11].chars().count().max(1) as i32);
    }
    if words.is_empty() { return Ok(String::new()); }
    heights.sort(); units.sort(); words.sort();
    let tolerance = (heights[heights.len()/2]*65/100).max(1);
    let unit = (units[units.len()/2]*8/10).max(1);
    let mut text = String::new();
    let mut i = 0;
    while i<words.len() {
        let mut j=i+1;
        while j<words.len() && words[j].0-words[i].0<=tolerance { j+=1; }
        words[i..j].sort_by_key(|w| w.1);
        let mut line=String::new();
        let mut end=None;
        for &(_,x,width,word) in &words[i..j] {
            let spaces = if end.is_none() || x-end.unwrap_or(0)>3*unit {
                (x/unit-line.chars().count() as i32).max(if end.is_some(){2}else{0})
            } else {1};
            line.push_str(&" ".repeat(spaces as usize));
            line.push_str(word); end=Some(x+width);
        }
        text.push_str(&line); text.push('\n'); i=j;
    }
    Ok(text)
}

pub struct Page {
    pub image: DynamicImage,
    pub words: Vec<crate::selection::Word>,
}

fn pdf_text(page: &poppler::Page, width: f64, height: f64) -> Result<(String, Vec<Word>), String> {
    let Some(text) = page.text() else { return Ok((String::new(), Vec::new())); };
    let mut boxes = std::ptr::null_mut();
    let mut count = 0;
    // Poppler allocates one rectangle per character. Copy before freeing with GLib.
    let boxes = unsafe {
        poppler::ffi::poppler_page_get_text_layout(page.to_glib_none().0, &mut boxes, &mut count);
        let values = if boxes.is_null() {
            Vec::new()
        } else {
            std::slice::from_raw_parts(boxes, count as usize).to_vec()
        };
        glib::ffi::g_free(boxes.cast());
        values
    };
    if text.chars().count() != boxes.len() {
        return Ok((text.to_string(), Vec::new()));
    }

    let scale = 2100.0 / width.max(height);
    let mut tsv = format!("header\n1\t1\t0\t0\t0\t0\t0\t0\t{}\t{}\t-1\tpage\n",
        (width * scale).ceil(), (height * scale).ceil());
    let mut line = 1;
    let mut token = String::new();
    let mut bounds: Option<(f64, f64, f64, f64)> = None;
    for (ch, rect) in text.chars().zip(boxes).chain(std::iter::once(('\n', poppler::ffi::PopplerRectangle {
        x1: 0., y1: 0., x2: 0., y2: 0.,
    }))) {
        if ch.is_whitespace() {
            if let Some((x1, y1, x2, y2)) = bounds.take() {
                use std::fmt::Write;
                writeln!(tsv, "5\t1\t0\t0\t{line}\t0\t{}\t{}\t{}\t{}\t100\t{token}",
                    (x1 * scale).round(), (y1 * scale).round(),
                    ((x2 - x1) * scale).round(), ((y2 - y1) * scale).round()).unwrap();
                token.clear();
            }
            if ch == '\n' { line += 1; }
        } else {
            token.push(ch);
            bounds = Some(match bounds {
                Some((x1, y1, x2, y2)) => (x1.min(rect.x1), y1.min(rect.y1), x2.max(rect.x2), y2.max(rect.y2)),
                None => (rect.x1, rect.y1, rect.x2, rect.y2),
            });
        }
    }
    Ok((layout_text(&tsv)?, selection::words(&tsv)))
}

fn render(page: &poppler::Page, width: f64, height: f64) -> Result<DynamicImage, String> {
    if !(width.is_finite() && height.is_finite() && width > 0. && height > 0.) {
        return Err("Ungültige PDF-Seitengröße.".into());
    }
    let scale = 2100.0 / width.max(height);
    let surface = cairo::ImageSurface::create(cairo::Format::Rgb24,
        (width * scale).ceil() as i32, (height * scale).ceil() as i32).map_err(|e| e.to_string())?;
    let context = cairo::Context::new(&surface).map_err(|e| e.to_string())?;
    context.set_source_rgb(1., 1., 1.);
    context.paint().map_err(|e| e.to_string())?;
    context.scale(scale, scale);
    page.render(&context);
    let mut bytes = Vec::new();
    surface.write_to_png(&mut bytes).map_err(|e| e.to_string())?;
    image::load_from_memory(&bytes).map_err(|e| e.to_string())
}

pub fn load(path: &Path) -> Result<(Vec<Page>, String), String> {
    let path = path.canonicalize().map_err(|e| e.to_string())?;
    let mut pages = Vec::new();
    let mut text = String::new();
    let mut ocr = None;
    if path.extension().and_then(OsStr::to_str).is_some_and(|s| s.eq_ignore_ascii_case("pdf")) {
        let uri = glib::filename_to_uri(&path, None).map_err(|e| e.to_string())?;
        let doc = poppler::Document::from_file(uri.as_str(), None).map_err(|e| e.to_string())?;
        for index in 0..doc.n_pages() {
            let page = doc.page(index).ok_or_else(|| format!("PDF-Seite {} fehlt.", index + 1))?;
            let (width, height) = page.size();
            let image = render(&page, width, height)?;
            let (mut part, mut words) = pdf_text(&page, width, height)?;
            if part.trim().chars().count() < 24 {
                (part, words) = recognize(&mut ocr, &image)?;
            }
            text.push_str(&part);
            text.push('\n');
            pages.push(Page { image, words });
        }
    } else {
        use image::ImageDecoder;
        let mut decoder = image::ImageReader::open(&path).map_err(|e| e.to_string())?
            .into_decoder().map_err(|e| e.to_string())?;
        let orientation = decoder.orientation().map_err(|e| e.to_string())?;
        let mut image = DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
        image.apply_orientation(orientation);
        let (width, height) = image.dimensions();
        if width.max(height) > 3200 { image = image.thumbnail(3200, 3200); }
        let (part, words) = recognize(&mut ocr, &image)?;
        text = part;
        pages.push(Page { image, words });
    }
    if pages.is_empty() { return Err("Keine lesbare Seite gefunden.".into()); }
    Ok((pages, text))
}

fn recognize(engine: &mut Option<crate::ocr::Engine>, image: &DynamicImage) -> Result<(String, Vec<Word>), String> {
    if engine.is_none() { *engine = Some(crate::ocr::Engine::new()?); }
    let tsv = engine.as_mut().unwrap().recognize(image)?;
    Ok((layout_text(&tsv)?, selection::words(&tsv)))
}

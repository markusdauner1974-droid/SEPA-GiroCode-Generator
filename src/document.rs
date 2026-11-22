use std::{ffi::OsStr, path::Path, process::Command};
use image::{DynamicImage, GenericImageView};

fn output(cmd: &mut Command) -> Result<Vec<u8>,String> {
    let name = cmd.get_program().to_string_lossy().into_owned();
    let result = cmd.output().map_err(|e| format!("{name}: {e}"))?;
    if !result.status.success() { return Err(format!("{name}: {}",String::from_utf8_lossy(&result.stderr).trim())); }
    Ok(result.stdout)
}

// Reconstruct physical rows even if OCR returns the two address columns in reading order.
fn ocr(path: &Path) -> Result<(String, Vec<crate::selection::Word>),String> {
    let installed = output(Command::new("tesseract").arg("--list-langs"))?;
    let installed = String::from_utf8_lossy(&installed);
    let langs = ["deu","eng"].into_iter().filter(|l| installed.lines().any(|s| s.trim()==*l)).collect::<Vec<_>>().join("+");
    if langs.is_empty() { return Err("Tesseract benötigt deutsche oder englische Sprachdaten.".into()); }
    let tsv = output(Command::new("tesseract").arg(path).args(["stdout","-l",&langs,"tsv"]))?;
    let tsv = String::from_utf8_lossy(&tsv);
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
    if words.is_empty() { return Ok((String::new(),Vec::new())); }
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
    Ok((text,crate::selection::words(&tsv)))
}

pub struct Page {
    pub image: DynamicImage,
    pub words: Vec<crate::selection::Word>,
}

pub fn load(path: &Path) -> Result<(Vec<Page>,String),String> {
    let path=path.canonicalize().map_err(|e| e.to_string())?;
    let mut pages=Vec::new();
    let mut text=String::new();
    let tmp=tempfile::tempdir().map_err(|e| e.to_string())?;
    let png=tmp.path().join("page.png");
    if path.extension().and_then(OsStr::to_str).is_some_and(|s| s.eq_ignore_ascii_case("pdf")) {
        let raw=output(Command::new("pdftotext").args(["-layout","-enc","UTF-8"]).arg(&path).arg("-"))?;
        let raw=String::from_utf8_lossy(&raw);
        for (i,part) in raw.split_terminator('\x0c').enumerate() {
            let n=(i+1).to_string();
            let bytes=output(Command::new("pdftoppm").args(["-f",&n,"-l",&n,"-singlefile","-scale-to","2100","-png"]).arg(&path))?;
            let words = if part.trim().chars().count()<24 {
                std::fs::write(&png,&bytes).map_err(|e| e.to_string())?;
                let (recognized,words) = ocr(&png)?;
                text.push_str(&recognized);
                words
            } else {
                text.push_str(part);
                let tsv = output(Command::new("pdftotext").args(["-f",&n,"-l",&n,"-tsv","-enc","UTF-8"]).arg(&path).arg("-"))?;
                crate::selection::words(&String::from_utf8_lossy(&tsv))
            };
            text.push('\n');
            pages.push(Page {image:image::load_from_memory(&bytes).map_err(|e| e.to_string())?,words});
        }
    } else {
        use image::ImageDecoder;
        let mut decoder=image::ImageReader::open(&path).map_err(|e| e.to_string())?.into_decoder().map_err(|e| e.to_string())?;
        let orientation=decoder.orientation().map_err(|e| e.to_string())?;
        let mut picture=DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
        picture.apply_orientation(orientation);
        let (w,h)=picture.dimensions();
        if w.max(h)>3200 { picture=picture.thumbnail(3200,3200); }
        picture.save(&png).map_err(|e| e.to_string())?;
        let (recognized,words) = ocr(&png)?;
        text = recognized;
        pages.push(Page {image:picture,words});
    }
    if pages.is_empty() {return Err("Keine lesbare Seite gefunden.".into());}
    Ok((pages,text))
}

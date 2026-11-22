use regex::Regex;
use std::collections::BTreeSet;

#[derive(Clone, Default)]
pub struct Payment {
    pub name: String,
    pub iban: String,
    pub bic: String,
    pub amount: String,
    pub purpose: String,
}

fn rx(p: &str) -> Regex { Regex::new(p).expect("static regular expression") }

fn unique(values: BTreeSet<String>) -> String {
    if values.len() == 1 { values.into_iter().next().unwrap() } else { String::new() }
}

fn field(text: &str, pattern: &str) -> String {
    unique(rx(pattern).captures_iter(text).map(|c| c[1].trim().to_owned()).collect())
}

fn iban_len(country: &str) -> usize {
    match country {
        "NO" => 15, "BE" => 16, "DK"|"FI"|"NL" => 18, "SI"|"MK" => 19,
        "AT"|"EE"|"LT" => 20, "CH"|"HR"|"LI"|"LV" => 21,
        "DE"|"BG"|"GB"|"IE"|"RS"|"ME" => 22, "GI" => 23,
        "AD"|"CZ"|"ES"|"MD"|"RO"|"SE"|"SK" => 24, "PT" => 25, "IS" => 26,
        "FR"|"GR"|"IT"|"MC"|"SM" => 27, "AL"|"CY"|"HU"|"PL" => 28,
        "MT" => 31, "VA" => 22, "LU" => 20, _ => 0
    }
}

fn valid_iban(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() < 5 || !s.is_ascii() || iban_len(&s[..2]) != b.len() ||
        !b[2..4].iter().all(u8::is_ascii_digit) { return false; }
    let mut rem = 0u32;
    for &c in b[4..].iter().chain(&b[..4]) {
        rem = match c {
            b'0'..=b'9' => rem * 10 + u32::from(c - b'0'),
            b'A'..=b'Z' => rem * 100 + u32::from(c - b'A' + 10),
            _ => return false,
        } % 97;
    }
    rem == 1
}

fn header_name(text: &str) -> String {
    let tax = rx(r"(?i)^USt[. -]*(Id|Ident)|^Umsatzsteuer-ID");
    let suffix = rx(r"(?i)\b(GmbH(?: & Co\. KG)?|AG|KG|OHG|GbR|e\.V\.)$");
    let cell = rx(r"\S+(?: \S+)*");
    let rows: Vec<_> = text.lines().take(35)
        .map(|line| cell.find_iter(line).map(|m| (line[..m.start()].chars().count(),m.as_str())).collect::<Vec<_>>()).collect();
    let mut names = BTreeSet::new();
    for (i,row) in rows.iter().enumerate() {
        for &(col,_) in row.iter().filter(|(_,s)| tax.is_match(s)) {
            let mut previous = "";
            for r in &rows[..i] {
                let s = r.iter().find(|(x,_)| x.abs_diff(col) <= 2).map_or("", |t| t.1);
                if let Some(m) = suffix.find(s) {
                    let name = if m.start() == 0 {
                        if previous.is_empty() { continue; }
                        format!("{previous} {s}")
                    } else { s.to_owned() };
                    if name.chars().count() <= 70 && !name.contains(':') { names.insert(name); }
                }
                previous = s;
            }
        }
    }
    unique(names)
}

pub fn external_payment_notice(text: &str) -> bool {
    rx(r"(?i)keine\s+Zahlung(?:en)?\s+auf\s+unser(?:e[smn]?)?\s+Konto\b").is_match(text)
}

// A line can contain several visual columns. Match a label and its value
// within that column, even when a document places the value on the next line.
struct Cell<'a> {
    column: usize,
    text: &'a str,
}

fn columns(text: &str) -> Vec<Vec<Cell<'_>>> {
    let cell = rx(r"\S+(?: \S+)*");
    text.lines().map(|row| cell.find_iter(row).map(|m| Cell {
        column: row[..m.start()].chars().count(),
        text: m.as_str(),
    }).collect()).collect()
}

fn labeled<'a>(rows: &'a [Vec<Cell<'a>>], pattern: &str) -> Vec<&'a str> {
    labeled_cells(rows, pattern, false)
}

fn labeled_cells<'a>(rows: &'a [Vec<Cell<'a>>], pattern: &str, across_row: bool) -> Vec<&'a str> {
    let label = rx(pattern);
    let mut values = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        for (column_index, cell) in row.iter().enumerate() {
            let Some(captures) = label.captures(cell.text) else { continue };
            let rest = captures.get(1).unwrap().as_str().trim();
            if !rest.is_empty() {
                values.push(rest);
            } else if across_row && column_index + 1 < row.len() {
                // A totals table often prints the amount at the right edge.
                // Let the amount parser reject rows with competing numbers.
                values.extend(row[column_index + 1..].iter().map(|value| value.text));
            } else if let Some(next) = rows.iter().skip(index + 1).take(2)
                .find_map(|row| row.iter().find(|next| next.column.abs_diff(cell.column) <= 3)) {
                // An intervening row can belong entirely to the other column.
                values.push(next.text);
            }
        }
    }
    values
}

fn ibans(text: &str) -> BTreeSet<String> {
    rx(r"(?i-u:\b[A-Z]{2}[ \t]?[0-9]{2})").find_iter(text).filter_map(|m| {
        let prefix = m.as_str()[..2].to_ascii_uppercase();
        let length = iban_len(&prefix);
        let candidate: String = text[m.start()..].chars()
            .filter(|c| !matches!(c, ' ' | '\t'))
            .take(length).map(|c| c.to_ascii_uppercase()).collect();
        valid_iban(&candidate).then_some(candidate)
    }).collect()
}

fn invoice_amount(rows: &[Vec<Cell<'_>>]) -> String {
    let number = rx(r"(?i)^(?:EUR|€)?\s*([0-9]{1,3}(?:\.[0-9]{3})+,[0-9]{2}|[0-9]+[,.][0-9]{2})\s*(?:EUR|€)?$");
    for pattern in [
        r"(?i)^(?:Restbetrag|Restzahlung|Noch zu zahlen)\s*[:#]?\s*(.*)$",
        r"(?i)^(?:(?:Amount due\s*/\s*)?(?:Zu zahlender Betrag|Zahlbetrag|Offener Betrag)|Amount due|F[äa]lliger Betrag|Zur Zahlung f[äa]llig)\s*[:#]?\s*(.*)$",
        r"(?i)^(?:Gesamtbetrag|Rechnungsbetrag|Rechnungssumme|Endsumme(?: inkl\. MwSt\.)?)(?:\s*\(brutto\))?\s*[:#]?\s*(.*)$",
    ] {
        let amounts: BTreeSet<_> = labeled_cells(rows, pattern, true).into_iter()
            .filter_map(|value| number.captures(value))
            .map(|m| if m[1].contains(',') { m[1].to_owned() } else { m[1].replace('.', ",") })
            .collect();
        if !amounts.is_empty() { return unique(amounts); }
    }
    String::new()
}

pub fn extract(text: &str) -> (Payment, Vec<String>) {
    let rows = columns(text);
    let old = labeled(&rows, r"(?i)^(?:Alte|Bisherige|Ungültige) IBAN(?:\s*\([^)]*\))?\s*[:#]?\s*(.*)$")
        .into_iter().flat_map(ibans).collect::<BTreeSet<_>>();
    let mut accounts = text.lines().flat_map(ibans).filter(|iban| !old.contains(iban)).collect::<BTreeSet<_>>();
    let preferred = labeled(&rows, r"(?i)^(?:Aktuelle|Neue) IBAN\s*[:#]?\s*(.*)$")
        .into_iter().flat_map(ibans).filter(|iban| !old.contains(iban)).collect::<BTreeSet<_>>();

    let names = labeled(&rows, r"(?i)^(?:(?:Beneficiary\s*/\s*)?(?:(?:Zahlungs)?empf(?:ä|a|[eé]a?)nger|Beg[üu]nstigter|Kontoinhaber(?:in)?|Konto lautend auf|Zahlbar an)|Beneficiary)\s*:\s*(.*)$");
    let name_values: BTreeSet<_> = names.into_iter().map(|name|
        rx(r"(?i)\s+(?:IBAN|BIC)\s*:").split(name).next().unwrap_or(name).trim())
        .filter(|name| !name.is_empty() && name.chars().count() <= 70 && !name.contains(':'))
        .map(str::to_owned).collect();
    let mut name = if name_values.is_empty() { header_name(text) } else { unique(name_values) };
    let amount = if rx(r"\b(CHF|USD|GBP|SEK|NOK|DKK|PLN|CZK)\b|[$£]").is_match(text) {
        String::new()
    } else { invoice_amount(&rows) };
    let purposes: BTreeSet<_> = labeled(&rows,
        r"(?i)^(?:Verwendungszweck|Zahlungsgrund|Zahlungsreferenz|Bei Zahlung angeben|Überweisungstext|Payment reference|Referenz bei Überweisung|Kassenzeichen)\s*:\s*(.*)$")
        .into_iter().filter(|value| !value.is_empty() && value.chars().count() <= 140)
        .map(str::to_owned).collect();
    let purpose = if purposes.is_empty() {
        let number = field(text,r"(?i)\b(?:Rechnungsnummer|Rechnungs[- ]?Nr\.?)[ \t:#]*([\w/.-]*\d[\w/.-]*)");
        if number.is_empty() { String::new() } else { format!("Rechnung {number}") }
    } else { unique(purposes) };

    // Seller details are inappropriate when the invoice directs payment elsewhere.
    let external = external_payment_notice(text);
    if external {
        accounts.clear();
        name.clear();
    }
    let iban = if !external && !preferred.is_empty() { unique(preferred) } else { unique(accounts.clone()) };
    let bic = if !iban.is_empty() && accounts.len() == 1 && old.is_empty() {
        field(text,r"\bBIC[ \t:]*([A-Z]{6}[A-Z0-9]{2}(?:[A-Z0-9]{3})?)\b")
    } else { String::new() };
    (Payment { name,iban,bic,amount,purpose }, accounts.into_iter().collect())
}

impl Payment {
    pub fn payload(&self) -> Result<String,String> {
        let name = self.name.trim();
        let purpose = self.purpose.trim();
        if name.is_empty() || name.chars().count()>70 || purpose.chars().count()>140 ||
            name.chars().chain(purpose.chars()).any(char::is_control) { return Err("Empfänger und Verwendungszweck prüfen.".into()); }
        let iban = self.iban.split_whitespace().collect::<String>().to_ascii_uppercase();
        if !valid_iban(&iban) { return Err("IBAN prüfen.".into()); }
        let bic = self.bic.trim().to_ascii_uppercase();
        let eea = "AT BE BG HR CY CZ DK EE FI FR DE GR HU IE IT LV LT LU MT NL PL PT RO SK SI ES SE IS LI NO";
        if (!bic.is_empty() || !eea.split_whitespace().any(|c| c == &iban[..2])) &&
            !rx(r"^[A-Z]{6}[A-Z0-9]{2}([A-Z0-9]{3})?$").is_match(&bic) { return Err("BIC prüfen.".into()); }
        let mut amount = self.amount.trim().to_string();
        if rx(r"^[0-9]{1,3}(\.[0-9]{3})+,[0-9]{2}$").is_match(&amount) { amount = amount.replace('.', ""); }
        let cents_error = "Positiven Betrag in Euro eingeben.";
        if !rx(r"^[0-9]{1,9}([.,][0-9]{1,2})?$").is_match(&amount) { return Err(cents_error.into()); }
        let (whole,frac) = amount.split_once(['.',',']).unwrap_or((&amount,""));
        let cents = whole.parse::<u64>().unwrap()*100 + format!("{frac:0<2}").parse::<u64>().unwrap();
        if cents == 0 { return Err(cents_error.into()); }
        let data = format!("BCD\n002\n1\nSCT\n{bic}\n{name}\n{iban}\nEUR{}.{:02}\n\n\n{purpose}", cents/100,cents%100);
        let data = data.trim_end_matches('\n').to_owned();
        if data.len()>331 { return Err("Verwendungszweck kürzen.".into()); }
        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn girocode() {
        let (p,_) = extract("Empfänger: Beispiel GmbH\nIBAN: DE89 3704 0044 0532 0130 00\nZahlbetrag: 1.234,50\nRechnungsnummer: T-17");
        assert_eq!(p.payload().unwrap(),"BCD\n002\n1\nSCT\n\nBeispiel GmbH\nDE89370400440532013000\nEUR1234.50\n\n\nRechnung T-17");
    }
    #[test]
    fn marketplace_invoice() {
        let text = "Empfänger: Beispielshop GmbH\nIBAN: DE89370400440532013000\nRechnungs Nr. TEST-17\nWarenwert (netto) EUR 25,20\nRechnungsbetrag (brutto) EUR 29,99\nBitte veranlassen Sie keine Zahlungen auf unser Konto.";
        let (payment, accounts) = extract(text);
        assert_eq!(payment.amount, "29,99");
        assert_eq!(payment.purpose, "Rechnung TEST-17");
        assert!(external_payment_notice(text));
        assert!(payment.name.is_empty() && payment.iban.is_empty() && accounts.is_empty());
        assert!(payment.payload().is_err());
    }
    #[test]
    fn columns() {
        assert_eq!(header_name("Kunde AG         Nordlicht Beratung\nEinkauf          GmbH & Co. KG\nOrt              USt-ID: TEST"),"Nordlicht Beratung GmbH & Co. KG");
    }
}

#[cfg(test)]
mod invoice_corpus {
    use super::extract;
    const EXPECTED: &str = include_str!("../tests/invoices/expectations.csv");
    const TEXTS: &str = include_str!("../tests/invoices/layout-corpus.txt");

    #[test]
    fn matches_pdf_and_ocr_examples() {
        let mut errors = Vec::new();
        for row in EXPECTED.lines().skip(1) {
            let expected: Vec<_> = row.split(';').collect();
            for extension in ["pdf", "png"] {
                let file = format!("{}.{extension}", expected[0]);
                let prefix = format!("@{file}\n");
                let text = TEXTS.split_once(&prefix).expect(&file).1.split("\n@").next().unwrap();
                let (actual, _) = extract(text);
                let fields = [actual.name, actual.iban, actual.amount, actual.purpose];
                for (index, got) in fields.iter().enumerate() {
                    if got != expected[index + 1] {
                        errors.push(format!("{file} Feld {}: {:?} statt {:?}", index + 1, got, expected[index + 1]));
                    }
                }
            }
        }
        assert!(errors.is_empty(), "{}", errors.join("\n"));
    }
}

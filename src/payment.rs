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
        .take_while(|s| !s.trim().eq_ignore_ascii_case("Rechnung"))
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

pub fn extract(text: &str) -> (Payment, Vec<String>) {
    let mut accounts = BTreeSet::new();
    for m in rx(r"\b[A-Z]{2}[0-9]{2}").find_iter(text) {
        let n = iban_len(&m.as_str()[..2]);
        let tail = &text[m.start()..];
        let mut chars = tail.chars().filter(|c| !matches!(c,' '|'\t'));
        let candidate: String = chars.by_ref().take(n).collect();
        if valid_iban(&candidate) { accounts.insert(candidate); }
    }
    let name_pattern = r"(?im)(?:^|[ \t]{2,})(?:Kontoinhaberin?|Kontoinhaber|Zahlungsempfänger|Empfänger)[ \t]*:[ \t]*(\S+(?: \S+)*)";
    let labels = rx(name_pattern);
    let mut name = if labels.is_match(text) { field(text,name_pattern) } else { header_name(text) };
    let amount = if rx(r"\b(CHF|USD|GBP|SEK|NOK|DKK|PLN|CZK)\b|[$£]").is_match(text) { String::new() } else {
        field(text,r"(?i)(?:zu zahlender Betrag|Zahlbetrag|offener Betrag|Gesamtbetrag|Rechnungsbetrag)(?:[ \t]*\([ \t]*brutto[ \t]*\))?[ \t:]*(?:EUR|€)?[ \t]*(\d+(?:\.\d{3})*,\d{2}|\d+\.\d{2})\b")
    };
    let mut purpose = field(text,r"(?im)^\s*(?:Verwendungszweck|Zahlungsgrund)[ \t]*:[ \t]*([^\n]+)");
    if purpose.is_empty() {
        let number = field(text,r"(?i)\b(?:Rechnungsnummer|Rechnungs[- ]?Nr\.?)[ \t:#]*([\w/.-]*\d[\w/.-]*)");
        if !number.is_empty() { purpose = format!("Rechnung {number}"); }
    }
    // A seller account must not be proposed when the invoice directs payment elsewhere.
    if external_payment_notice(text) {
        accounts.clear();
        name.clear();
    }
    let iban = unique(accounts.clone());
    let bic = if iban.is_empty() { String::new() } else { field(text,r"\bBIC[ \t:]*([A-Z]{6}[A-Z0-9]{2}(?:[A-Z0-9]{3})?)\b") };
    (Payment { name,iban,bic,amount,purpose },accounts.into_iter().collect())
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

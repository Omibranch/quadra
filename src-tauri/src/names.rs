//! Provider-supplied text arrives full of emoji. The interface draws its own flags and uses none,
//! so the country is read out of the flag and every pictograph is dropped.

fn is_pictograph(c: char) -> bool {
    matches!(c as u32,
        0x1F000..=0x1FAFF | 0x2600..=0x27BF | 0x2300..=0x23FF | 0x2B00..=0x2BFF | 0x2190..=0x21FF
        | 0xFE00..=0xFE0F | 0x200B..=0x200D | 0x20E3 | 0xE0020..=0xE007F | 0x3030 | 0x303D | 0x3297 | 0x3299
        | 0x00A9 | 0x00AE | 0x2122 | 0x2139 | 0x25A0..=0x25FF | 0x2460..=0x24FF | 0xFEFF)
}

/// Country code spelled by the first flag in the text.
pub fn flag_cc(s: &str) -> Option<String> {
    let mut prev: Option<char> = None;
    for c in s.chars() {
        let u = c as u32;
        if (0x1F1E6..=0x1F1FF).contains(&u) {
            let letter = (b'A' + (u - 0x1F1E6) as u8) as char;
            if let Some(p) = prev {
                return Some(format!("{p}{letter}"));
            }
            prev = Some(letter);
        } else {
            prev = None;
        }
    }
    None
}

pub fn clean(s: &str) -> String {
    let stripped: String = s.chars().map(|c| if is_pictograph(c) { ' ' } else { c }).collect();
    let edge = |c: char| c.is_whitespace() || matches!(c, '|' | '-' | '—' | '–' | '·' | ',' | ':' | '•');
    let mut out = String::new();
    for line in stripped.lines() {
        let mut t = line.split_whitespace().collect::<Vec<_>>().join(" ");
        while t.contains("| |") {
            t = t.replace("| |", "|");
        }
        let t = t.trim_matches(edge);
        if !t.is_empty() {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(t);
        }
    }
    out
}

/// Countries providers name without a flag.
pub fn guess_cc(name: &str) -> Option<String> {
    const WORDS: &[(&str, &str)] = &[
        ("герман", "DE"), ("german", "DE"), ("нидерланд", "NL"), ("netherland", "NL"), ("голланд", "NL"),
        ("финлянд", "FI"), ("finland", "FI"), ("швец", "SE"), ("sweden", "SE"), ("польш", "PL"), ("poland", "PL"),
        ("франц", "FR"), ("france", "FR"), ("великобритан", "GB"), ("англи", "GB"), ("united kingdom", "GB"),
        ("сша", "US"), ("usa", "US"), ("united states", "US"), ("америк", "US"), ("швейцар", "CH"), ("switzerland", "CH"),
        ("турц", "TR"), ("turkey", "TR"), ("казахстан", "KZ"), ("kazakhstan", "KZ"), ("росси", "RU"), ("russia", "RU"),
        ("япон", "JP"), ("japan", "JP"), ("сингапур", "SG"), ("singapore", "SG"), ("эстон", "EE"), ("estonia", "EE"),
        ("латв", "LV"), ("latvia", "LV"), ("литв", "LT"), ("lithuania", "LT"), ("австри", "AT"), ("austria", "AT"),
        ("испан", "ES"), ("spain", "ES"), ("итал", "IT"), ("italy", "IT"), ("канад", "CA"), ("canada", "CA"),
        ("инди", "IN"), ("india", "IN"), ("гонконг", "HK"), ("hong kong", "HK"), ("оаэ", "AE"), ("эмират", "AE"),
        ("чехи", "CZ"), ("czech", "CZ"), ("норвег", "NO"), ("norway", "NO"), ("дани", "DK"), ("denmark", "DK"),
        ("украин", "UA"), ("ukraine", "UA"), ("молдов", "MD"), ("армени", "AM"), ("грузи", "GE"), ("израил", "IL"),
        ("австрал", "AU"), ("australia", "AU"), ("бразил", "BR"), ("brazil", "BR"), ("коре", "KR"), ("korea", "KR"),
        ("румын", "RO"), ("romania", "RO"), ("болгар", "BG"), ("bulgaria", "BG"), ("серби", "RS"), ("serbia", "RS"),
        ("ирланд", "IE"), ("ireland", "IE"), ("бельги", "BE"), ("belgium", "BE"), ("португал", "PT"), ("венгри", "HU"),
    ];
    let low = name.to_lowercase();
    WORDS.iter().find(|(w, _)| low.contains(w)).map(|(_, cc)| cc.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_flag_and_drops_emoji() {
        assert_eq!(flag_cc("🇩🇪 ⚡️ Германия").as_deref(), Some("DE"));
        assert_eq!(clean("🇩🇪 ⚡️ Германия"), "Германия");
        assert_eq!(clean("🇺🇸 США | 🤖 Gemini"), "США | Gemini");
        assert_eq!(clean("🇵🇱 Польша Резерв 🛟"), "Польша Резерв");
        assert_eq!(clean("⚠️Не грузит - выбери другую страну 👇🏼"), "Не грузит - выбери другую страну");
        assert_eq!(flag_cc("Обход №1.0"), None);
        assert_eq!(guess_cc("Швеция 2").as_deref(), Some("SE"));
    }
}

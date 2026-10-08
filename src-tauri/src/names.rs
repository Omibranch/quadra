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
///
/// A word of the name has to start with the stem: "не грузит" is not Georgia and "индивидуальный"
/// is not India. Short ones that are words in their own right ("usa") have to be the whole word.
pub fn guess_cc(name: &str) -> Option<String> {
    const STEMS: &[(&str, &str)] = &[
        ("герман", "DE"), ("german", "DE"), ("нидерланд", "NL"), ("netherland", "NL"), ("голланд", "NL"),
        ("финлянд", "FI"), ("finland", "FI"), ("швеци", "SE"), ("sweden", "SE"), ("польш", "PL"), ("poland", "PL"),
        ("франци", "FR"), ("france", "FR"), ("великобритан", "GB"), ("британи", "GB"), ("britain", "GB"),
        ("америк", "US"), ("швейцар", "CH"), ("switzerland", "CH"),
        ("турци", "TR"), ("turkey", "TR"), ("türkiye", "TR"), ("казахстан", "KZ"), ("kazakhstan", "KZ"), ("росси", "RU"), ("russia", "RU"),
        ("япони", "JP"), ("japan", "JP"), ("сингапур", "SG"), ("singapore", "SG"), ("эстони", "EE"), ("estonia", "EE"),
        ("латви", "LV"), ("latvia", "LV"), ("литв", "LT"), ("lithuania", "LT"), ("австри", "AT"), ("austria", "AT"),
        ("испани", "ES"), ("spain", "ES"), ("итали", "IT"), ("italy", "IT"), ("канад", "CA"), ("canada", "CA"),
        ("гонконг", "HK"), ("hongkong", "HK"), ("эмират", "AE"), ("emirates", "AE"),
        ("czech", "CZ"), ("норвеги", "NO"), ("norway", "NO"), ("denmark", "DK"),
        ("украин", "UA"), ("ukraine", "UA"), ("молдов", "MD"), ("moldova", "MD"), ("армени", "AM"), ("armenia", "AM"),
        ("израил", "IL"), ("israel", "IL"), ("австрали", "AU"), ("australia", "AU"), ("бразили", "BR"), ("brazil", "BR"),
        ("румыни", "RO"), ("romania", "RO"), ("болгари", "BG"), ("bulgaria", "BG"), ("serbia", "RS"),
        ("ирланд", "IE"), ("ireland", "IE"), ("бельги", "BE"), ("belgium", "BE"), ("португал", "PT"), ("portugal", "PT"),
        ("венгри", "HU"), ("hungary", "HU"), ("georgia", "GE"), ("india", "IN"), ("korea", "KR"),
    ];
    // every form the word takes, for the stems that also begin other words
    const WORDS: &[(&str, &str)] = &[
        ("сша", "US"), ("usa", "US"), ("оаэ", "AE"), ("uae", "AE"),
        ("англия", "GB"), ("англии", "GB"), ("england", "GB"),
        ("грузия", "GE"), ("грузии", "GE"), ("индия", "IN"), ("индии", "IN"), ("дания", "DK"), ("дании", "DK"),
        ("корея", "KR"), ("кореи", "KR"), ("чехия", "CZ"), ("чехии", "CZ"), ("сербия", "RS"), ("сербии", "RS"),
    ];
    const PHRASES: &[(&str, &str)] = &[("united kingdom", "GB"), ("united states", "US"), ("hong kong", "HK")];

    let low = name.to_lowercase();
    if let Some((_, cc)) = PHRASES.iter().find(|(p, _)| low.contains(p)) {
        return Some(cc.to_string());
    }
    for word in low.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()) {
        if let Some((_, cc)) = WORDS.iter().find(|(w, _)| word == *w) {
            return Some(cc.to_string());
        }
        if let Some((_, cc)) = STEMS.iter().find(|(st, _)| word.starts_with(st)) {
            return Some(cc.to_string());
        }
    }
    None
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
        assert_eq!(guess_cc("США (для ИИ)").as_deref(), Some("US"));
        assert_eq!(guess_cc("Сервер в Грузии").as_deref(), Some("GE"));
        assert_eq!(guess_cc("United Kingdom #3").as_deref(), Some("GB"));
        // words that merely look like a country
        assert_eq!(guess_cc("Не грузит - выбери другую страну"), None);
        assert_eq!(guess_cc("Индивидуальный сервер"), None);
        assert_eq!(guess_cc("Usage limit"), None);
        assert_eq!(guess_cc("Корень"), None);
    }
}

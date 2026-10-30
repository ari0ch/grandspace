//! Procedural names for systems, regions and factions.

use crate::model::Power;
use crate::rng::Rng;

const ONSETS: &[&str] = &[
    "K", "V", "T", "S", "M", "R", "D", "H", "Br", "Kr", "St", "Th", "Cal", "Or", "Pel", "Ash",
    "Ver", "Mar", "Hal", "Ny", "Ser", "Tam", "Ul", "Zar", "Eb", "Ix",
];
const VOWELS: &[&str] = &["a", "e", "i", "o", "u", "ae", "ei", "y"];
const CODAS: &[&str] = &[
    "n", "r", "s", "th", "x", "ll", "rn", "st", "d", "m", "nd", "ck", "", "", "",
];
const SUFFIXES: &[&str] = &[
    "", "", "", "", "'s Reach", " Prime", " Gate", " Drift", " Deep", " Hold", " Station",
];

pub fn word(rng: &mut Rng, syllables: usize) -> String {
    let mut s = String::new();
    for i in 0..syllables {
        let onset = rng.pick(ONSETS).unwrap();
        if i == 0 {
            s.push_str(onset);
        } else {
            s.push_str(&onset.to_lowercase());
        }
        s.push_str(rng.pick(VOWELS).unwrap());
        s.push_str(rng.pick(CODAS).unwrap());
    }
    s
}

pub fn system_name(rng: &mut Rng) -> String {
    let mut name = String::new();
    while name.len() > 10 || name.len() < 3 {
        let syllables = rng.range_usize(1, 3);
        name = word(rng, syllables);
    }
    name.push_str(rng.pick(SUFFIXES).unwrap());
    name
}

const REGION_WORDS: &[&str] = &[
    "Reach", "Expanse", "Marches", "Verge", "Shallows", "Belt", "Frontier", "Drift", "Core",
    "Hollows",
];

pub fn region_name(rng: &mut Rng) -> String {
    format!("{} {}", word(rng, 2), rng.pick(REGION_WORDS).unwrap())
}

pub fn roman(n: usize) -> &'static str {
    const R: [&str; 10] = ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"];
    R.get(n).copied().unwrap_or("X")
}

const SAINTS: &[&str] = &[
    "Ines", "Teodor", "Mara", "Aurelio", "Benedek", "Clare", "Josef", "Okoye", "Halvard", "Sabine",
];
const TRADES: &[&str] = &[
    "Dockworkers",
    "Miners",
    "Haulers",
    "Shipwrights",
    "Reactor Techs",
    "Hydroponics",
    "Drillers",
    "Teamsters",
];

pub fn faction_name(rng: &mut Rng, power: Power) -> String {
    match power {
        Power::Mafia => format!("The {} Family", word(rng, 2)),
        Power::Church => match rng.range_usize(0, 2) {
            0 => format!("Order of St. {}", rng.pick(SAINTS).unwrap()),
            _ => format!("Diocese of {}", word(rng, 2)),
        },
        Power::Unions => format!(
            "{} Local {}",
            rng.pick(TRADES).unwrap(),
            rng.range_usize(2, 99)
        ),
    }
}

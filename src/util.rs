use std::borrow::Cow;
use std::env;
use std::ffi::OsStr;
use std::io::{self, IsTerminal};
use std::iter;
use std::mem;
use std::path::Path;

use clap::ColorChoice;
use ring::digest::{digest, SHA256};
use yansi::Paint;

use crate::config::Config;

/// Prints a warning.
macro_rules! warnln {
    ( $( $arg:tt )* ) => {
        if !$crate::QUIET.load(std::sync::atomic::Ordering::Relaxed) {
            use std::io::Write;
            let mut stderr = std::io::stderr().lock();
            write!(stderr, "{} ", yansi::Paint::new("warning:").fg(yansi::Color::Yellow).bold())?;
            writeln!(stderr, $($arg)*)?;
        }
    };
}

/// Prints a status message with a trailing newline.
macro_rules! infoln {
    ( $( $arg:tt )* ) => {
        if !$crate::QUIET.load(std::sync::atomic::Ordering::Relaxed) {
            use std::io::Write;
            let mut stderr = std::io::stderr().lock();
            write!(stderr, "{} ", yansi::Paint::new("info:").fg(yansi::Color::Cyan).bold())?;
            writeln!(stderr, $($arg)*)?;
        }
    };
}

/// Prints a status message without a trailing newline.
macro_rules! info_start {
    ( $( $arg:tt )* ) => {
        if !$crate::QUIET.load(std::sync::atomic::Ordering::Relaxed) {
            use std::io::Write;
            let mut stderr = std::io::stderr().lock();
            write!(stderr, "{} ", yansi::Paint::new("info:").fg(yansi::Color::Cyan).bold())?;
            write!(stderr, $($arg)*)?;
        }
    };
}

/// End the status message started using `info_start`.
macro_rules! info_end {
    ( $( $arg:tt )* ) => {
        if !$crate::QUIET.load(std::sync::atomic::Ordering::Relaxed) {
            use std::io::Write;
            writeln!(std::io::stderr(), $($arg)*)?;
        }
    };
}

pub(crate) use {info_end, info_start, infoln, warnln};

/// Get languages from environment variables according to the tldr client specification.
fn get_languages_from_env() -> Vec<String> {
    // https://github.com/tldr-pages/tldr/blob/main/CLIENT-SPECIFICATION.md#language

    let var_lang = env::var("LANG").ok();
    let var_language = env::var("LANGUAGE").ok();

    if var_lang.is_none() {
        return vec!["en".to_string()];
    }

    let var_lang = var_lang.unwrap();
    let var_language = var_language.as_deref();

    let mut result = vec![];
    let languages = var_language
        .unwrap_or("")
        .split(':')
        .chain(iter::once(&*var_lang));

    for lang in languages {
        if lang.len() >= 5 && lang.chars().nth(2) == Some('_') {
            // <language>_<country> (ll_CC - 5 characters)
            result.push(&lang[..5]);
            // <language> (ll - 2 characters)
            result.push(&lang[..2]);
        } else if lang.len() == 2 {
            result.push(lang);
        }
    }

    result.push("en");

    result.into_iter().map(String::from).collect()
}

/// Return languages from the config + English or run `get_languages_from_env()` if the language config is empty.
pub fn get_languages(config: &mut Config) -> Vec<String> {
    if config.cache.languages.is_empty() {
        get_languages_from_env()
    } else {
        // English pages should always be downloaded and searched.
        config.cache.languages.push("en".to_string());
        config.cache.languages.clone()
    }
}

/// Prepend `pages.` to each `String`.
pub fn languages_to_langdirs(languages: &[String]) -> Vec<String> {
    languages
        .iter()
        .map(|lang| format!("pages.{lang}"))
        .collect()
}

/// Initialize color outputting.
pub fn init_color(color_mode: ColorChoice) {
    #[cfg(target_os = "windows")]
    let color_support = Paint::enable_windows_ascii();

    match color_mode {
        ColorChoice::Always => {}
        ColorChoice::Never => Paint::disable(),
        ColorChoice::Auto => {
            #[cfg(not(target_os = "windows"))]
            let color_support = true;
            let no_color = env::var_os("NO_COLOR").is_some_and(|x| !x.is_empty());

            if !color_support || no_color || !io::stdout().is_terminal() {
                Paint::disable();
            }
        }
    }
}

pub trait Dedup {
    /// Deduplicate a vector in place preserving the order of elements.
    fn dedup_nosort(&mut self);
}

impl<T> Dedup for Vec<T>
where
    T: PartialEq,
{
    fn dedup_nosort(&mut self) {
        let old = mem::replace(self, Vec::with_capacity(self.len()));
        for x in old {
            if !self.contains(&x) {
                self.push(x);
            }
        }
    }
}

pub trait PagePathExt {
    /// Extracts the page name from its path.
    fn page_name(&self) -> Option<Cow<str>>;
    /// Extracts the platform from the page path.
    fn page_platform(&self) -> Option<Cow<str>>;
}

impl PagePathExt for Path {
    fn page_name(&self) -> Option<Cow<str>> {
        self.file_stem().map(OsStr::to_string_lossy)
    }

    fn page_platform(&self) -> Option<Cow<str>> {
        self.parent()
            .and_then(|parent| parent.file_name().map(OsStr::to_string_lossy))
    }
}

/// Calculates the SHA256 hash and returns a hexadecimal string.
pub fn sha256_hexdigest(data: &[u8]) -> String {
    let digest = digest(&SHA256, data);
    let mut hex = String::new();

    for part in digest.as_ref() {
        hex.push_str(&format!("{part:02x}"));
    }

    hex
}

const DAY: u64 = 86400;
const HOUR: u64 = 3600;
const MINUTE: u64 = 60;

/// Convert time in seconds to a human-readable `String`.
pub fn duration_fmt(secs: u64) -> String {
    let days = secs / DAY;
    let hours = (secs % DAY) / HOUR;

    if days == 0 {
        let minutes = ((secs % DAY) % HOUR) / MINUTE;

        if hours == 0 {
            if minutes == 0 {
                format!("{secs}s")
            } else {
                let seconds = secs % MINUTE;

                if seconds == 0 {
                    format!("{minutes}min")
                } else {
                    format!("{minutes}min, {seconds}s")
                }
            }
        } else if minutes == 0 {
            format!("{hours}h")
        } else {
            format!("{hours}h, {minutes}min")
        }
    } else if hours == 0 {
        format!("{days}d")
    } else {
        format!("{days}d, {hours}h")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn prepare_env(lang: Option<&str>, language: Option<&str>) {
        if let Some(lang) = lang {
            env::set_var("LANG", lang);
        } else {
            env::remove_var("LANG");
        }

        if let Some(language) = language {
            env::set_var("LANGUAGE", language);
        } else {
            env::remove_var("LANGUAGE");
        }
    }

    #[test]
    fn env_languages() {
        // These vectors contain duplicates - de-dupping is done in cache.update()
        // and cache.find(), because update() requires a sorted vector, whereas
        // find() - an unsorted one.
        prepare_env(Some("cz"), Some("it:cz:de"));
        assert_eq!(get_languages_from_env(), ["it", "cz", "de", "cz", "en"]);

        prepare_env(Some("cz"), Some("it:de:fr"));
        assert_eq!(get_languages_from_env(), ["it", "de", "fr", "cz", "en"]);

        prepare_env(Some("it"), None);
        assert_eq!(get_languages_from_env(), ["it", "en"]);

        prepare_env(None, Some("it:cz"));
        assert_eq!(get_languages_from_env(), ["en"]);

        prepare_env(None, None);
        assert_eq!(get_languages_from_env(), ["en"]);

        prepare_env(Some("en_US.UTF-8"), Some("de_DE.UTF-8:pl:en"));
        assert_eq!(
            get_languages_from_env(),
            ["de_DE", "de", "pl", "en", "en_US", "en", "en"]
        );
    }

    #[test]
    fn sha256() {
        assert_eq!(
            sha256_hexdigest(b"This is a test."),
            "a8a2f6ebe286697c527eb35a58b5539532e9b3ae3b64d4eb0a46fb657b41562c"
        );
    }

    #[test]
    fn dur_fmt() {
        const SECOND: u64 = 1;

        assert_eq!(duration_fmt(SECOND), "1s");

        assert_eq!(duration_fmt(MINUTE), "1min");
        assert_eq!(duration_fmt(MINUTE + SECOND), "1min, 1s");

        assert_eq!(duration_fmt(HOUR), "1h");
        assert_eq!(duration_fmt(HOUR + SECOND), "1h");
        assert_eq!(duration_fmt(HOUR + MINUTE), "1h, 1min");
        assert_eq!(duration_fmt(HOUR + MINUTE + SECOND), "1h, 1min");

        assert_eq!(duration_fmt(DAY), "1d");
        assert_eq!(duration_fmt(DAY + SECOND), "1d");
        assert_eq!(duration_fmt(DAY + HOUR), "1d, 1h");
        assert_eq!(duration_fmt(DAY + HOUR + SECOND), "1d, 1h");
    }
}

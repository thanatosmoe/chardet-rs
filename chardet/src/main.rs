//! `chardetect` command-line tool, mirroring `chardet.cli`.

use chardet::{DetectOptions, DetectionResult};
use std::io::Read;
use std::process::ExitCode;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn era_from_name(name: &str) -> Option<u8> {
    Some(match name {
        "modern_web" => 1,
        "legacy_iso" => 2,
        "legacy_mac" => 4,
        "legacy_regional" => 8,
        "dos" => 16,
        "mainframe" => 32,
        "all" => 63,
        _ => return None,
    })
}

struct Args {
    files: Vec<String>,
    minimal: bool,
    language: bool,
    mime_type: bool,
    encoding_era: u8,
    include: Option<Vec<String>>,
    exclude: Option<Vec<String>>,
    no_match: String,
    empty_input: String,
    raw: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        files: Vec::new(),
        minimal: false,
        language: false,
        mime_type: false,
        encoding_era: 63,
        include: None,
        exclude: None,
        no_match: "cp1252".to_string(),
        empty_input: "utf-8".to_string(),
        raw: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--minimal" => args.minimal = true,
            "--raw" => args.raw = true,
            "-l" | "--language" => args.language = true,
            "-m" | "--mime-type" => args.mime_type = true,
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            "--version" => {
                println!("chardet {VERSION}");
                std::process::exit(0);
            }
            "-e" | "--encoding-era" => {
                let v = it.next().ok_or("--encoding-era requires a value")?;
                args.encoding_era = era_from_name(&v).ok_or_else(|| {
                    format!(
                        "invalid encoding era {v:?}; expected one of modern_web, legacy_iso, legacy_mac, legacy_regional, dos, mainframe, all"
                    )
                })?;
            }
            "-i" | "--include-encodings" => {
                let v = it.next().ok_or("--include-encodings requires a value")?;
                args.include = Some(v.split(',').map(|s| s.trim().to_string()).collect());
            }
            "-x" | "--exclude-encodings" => {
                let v = it.next().ok_or("--exclude-encodings requires a value")?;
                args.exclude = Some(v.split(',').map(|s| s.trim().to_string()).collect());
            }
            "--no-match-encoding" => {
                args.no_match = it.next().ok_or("--no-match-encoding requires a value")?;
            }
            "--empty-input-encoding" => {
                args.empty_input = it
                    .next()
                    .ok_or("--empty-input-encoding requires a value")?;
            }
            other if other.starts_with('-') && other != "-" => {
                return Err(format!("unknown option {other}"));
            }
            other => args.files.push(other.to_string()),
        }
    }
    Ok(args)
}

fn print_help() {
    println!(
        "usage: chardetect [-h] [--minimal] [-l] [-m] [-e ERA]\n\
         \x20                 [-i ENCODINGS] [-x ENCODINGS]\n\
         \x20                 [--no-match-encoding ENC] [--empty-input-encoding ENC]\n\
         \x20                 [--version] [files ...]\n\n\
         Detect character encoding of files."
    );
}

fn print_result(r: &DetectionResult, label: &str, args: &Args) {
    if args.raw {
        println!(
            "{}\t{}\t{}\t{}\t{}",
            label,
            r.encoding.as_deref().unwrap_or("None"),
            r.confidence,
            r.language.as_deref().unwrap_or("None"),
            r.mime_type.as_deref().unwrap_or("None"),
        );
        return;
    }
    let enc = r.encoding.clone().unwrap_or_else(|| "None".to_string());
    if args.minimal {
        let mut desc = enc;
        if args.language {
            desc += &format!(" {}", r.language.as_deref().unwrap_or("und"));
        }
        if args.mime_type {
            desc += &format!(
                " {}",
                r.mime_type.as_deref().unwrap_or("application/octet-stream")
            );
        }
        println!("{desc}");
    } else {
        let mut desc = enc;
        if args.language {
            let iso = r.language.as_deref().unwrap_or("und");
            let name = chardet::utils::language_name(iso)
                .map(titlecase)
                .unwrap_or_else(|| iso.to_string());
            desc += &format!(" {iso} ({name})");
        }
        if args.mime_type {
            desc += &format!(
                " {}",
                r.mime_type.as_deref().unwrap_or("application/octet-stream")
            );
        }
        println!("{label}: {desc} with confidence {}", r.confidence);
    }
}

fn titlecase(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

fn detect_one(data: &[u8], args: &Args) -> Result<DetectionResult, String> {
    let opts = DetectOptions {
        encoding_era: args.encoding_era,
        include_encodings: args.include.clone(),
        exclude_encodings: args.exclude.clone(),
        no_match_encoding: args.no_match.clone(),
        empty_input_encoding: args.empty_input.clone(),
        ..DetectOptions::default()
    };
    chardet::detect_with(data, &opts)
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("chardetect: {e}");
            return ExitCode::from(2);
        }
    };

    if args.files.is_empty() {
        let mut buf = Vec::new();
        if std::io::stdin().read_to_end(&mut buf).is_err() {
            eprintln!("chardetect: failed to read stdin");
            return ExitCode::from(1);
        }
        let data = &buf[..buf.len().min(chardet::utils::DEFAULT_MAX_BYTES)];
        match detect_one(data, &args) {
            Ok(r) => print_result(&r, "stdin", &args),
            Err(e) => {
                eprintln!("chardetect: stdin: detection failed: {e}");
                return ExitCode::from(1);
            }
        }
        return ExitCode::SUCCESS;
    }

    let mut errors = 0usize;
    for path in &args.files {
        match std::fs::read(path) {
            Ok(buf) => {
                let data = &buf[..buf.len().min(chardet::utils::DEFAULT_MAX_BYTES)];
                match detect_one(data, &args) {
                    Ok(r) => print_result(&r, path, &args),
                    Err(e) => {
                        eprintln!("chardetect: {path}: detection failed: {e}");
                        errors += 1;
                    }
                }
            }
            Err(e) => {
                eprintln!("chardetect: {path}: {e}");
                errors += 1;
            }
        }
    }
    if errors == args.files.len() {
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

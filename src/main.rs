use std::{
    env,
    fs::File,
    io::{self, BufRead, BufReader},
    process,
};

use anyhow::{anyhow, bail, Result};
use colored::Colorize;

use grep_starter_rust::match_regex;

fn main() -> Result<()> {
    let mut args = env::args().skip(1);

    let flag = args.next();
    if flag.is_none() || flag.unwrap() != "-E" {
        bail!("Expected first argument to be '-E'");
    }

    let pattern = args
        .next()
        .ok_or_else(|| anyhow!("Expected second argument to be a pattern"))?;

    fn process_lines<R: BufRead>(
        reader: R,
        pattern: &str,
        filename_prefix: Option<&str>,
    ) -> Result<usize> {
        let mut match_count = 0;
        let prefix = filename_prefix.map(|s| format!("{s}:")).unwrap_or_default();

        for line in reader.lines() {
            let line = line?;
            if let Some(matched) = match_regex(&line, pattern) {
                match_count += 1;

                let output = match line.find(&matched) {
                    Some(start) => {
                        let end = start + matched.len();
                        format!(
                            "{}{}{}{}",
                            prefix,
                            &line[..start],
                            &line[start..end].bright_red().bold(),
                            &line[end..]
                        )
                    }
                    None => format!("{prefix}{line}"),
                };

                println!("{output}");
            }
        }

        Ok(match_count)
    }

    let files = args.collect::<Vec<_>>();
    let match_count = if files.is_empty() {
        let stdin = io::stdin();
        let reader = BufReader::new(stdin.lock());
        process_lines(reader, &pattern, None)?
    } else {
        let mut total = 0;
        let show_prefix = files.len() > 1;
        for file_name in files {
            let file = File::open(&file_name)?;
            let reader = BufReader::new(file);
            let prefix = show_prefix.then_some(file_name.as_str());
            total += process_lines(reader, &pattern, prefix)?;
        }
        total
    };

    process::exit(if match_count > 0 { 0 } else { 1 });
}

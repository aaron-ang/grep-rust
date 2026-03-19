use std::{
    collections::BTreeMap,
    fs::File,
    io::{self, BufWriter, Read, Write},
    path::{Path, PathBuf},
    thread,
};

use anyhow::{bail, Context, Result};
use crossbeam_channel::{bounded, unbounded, Receiver};
use memmap2::Mmap;

use crate::{find_all_regex_spans_compiled, CompiledRegex, LineCandidate, RegexMatch};

const ANSI_BOLD_RED: &[u8] = b"\x1b[1;31m";
const ANSI_RESET: &[u8] = b"\x1b[0m";
const JOB_CHANNEL_BOUND: usize = 64;
const MMAP_THRESHOLD_BYTES: u64 = 1 << 20;

#[derive(Clone, Copy, Debug)]
pub struct SearchConfig {
    pub only_matching: bool,
    pub use_color: bool,
    pub show_prefix: bool,
    pub threads: usize,
}

#[derive(Debug, Clone)]
struct FileJob {
    sequence_no: usize,
    path: PathBuf,
}

#[derive(Debug)]
struct FileResult {
    sequence_no: usize,
    match_count: usize,
    rendered_output: Vec<u8>,
    error: Option<String>,
}

enum FileContent {
    Owned(String),
    Mapped(Mmap),
}

impl FileContent {
    fn as_str(&self, path: &Path) -> Result<&str> {
        match self {
            Self::Owned(text) => Ok(text),
            Self::Mapped(mmap) => std::str::from_utf8(mmap)
                .with_context(|| format!("{} is not valid UTF-8", path.display())),
        }
    }
}

#[derive(Default)]
struct WorkerBuffers {
    text: String,
    output: Vec<u8>,
}

impl WorkerBuffers {
    fn reset_output(&mut self) {
        self.output.clear();
    }
}

#[doc(hidden)]
pub fn run_search(
    files: &[PathBuf],
    recursive: bool,
    compiled: &CompiledRegex,
    config: SearchConfig,
) -> Result<usize> {
    let stdout = io::stdout();
    let mut writer = BufWriter::new(stdout.lock());
    run_search_to_writer(&mut writer, files, recursive, compiled, config)
}

#[doc(hidden)]
pub fn run_search_to_writer<W: Write>(
    writer: &mut W,
    files: &[PathBuf],
    recursive: bool,
    compiled: &CompiledRegex,
    config: SearchConfig,
) -> Result<usize> {
    let match_count = if files.is_empty() {
        let mut input = String::new();
        io::stdin().read_to_string(&mut input)?;
        let mut output = Vec::new();
        let total = search_text_content(&input, compiled, &config, "", &mut output);
        writer.write_all(&output)?;
        total
    } else {
        let file_paths = collect_files(files, recursive)?;
        let run_config = SearchConfig {
            show_prefix: file_paths.len() > 1,
            ..config
        };
        if file_paths.len() > 1 && run_config.threads > 1 {
            run_files_parallel(writer, file_paths, compiled, run_config)?
        } else {
            run_files_serial(writer, &file_paths, compiled, &run_config)?
        }
    };

    writer.flush()?;
    Ok(match_count)
}

fn run_files_serial<W: Write>(
    writer: &mut W,
    file_paths: &[PathBuf],
    compiled: &CompiledRegex,
    config: &SearchConfig,
) -> Result<usize> {
    let mut total = 0;
    let mut buffers = WorkerBuffers::default();

    for path in file_paths {
        let prefix = display_prefix(path, config.show_prefix);
        buffers.reset_output();
        let content = load_file_content(path, &mut buffers.text)?;
        let match_count = search_text_content(
            content.as_str(path)?,
            compiled,
            config,
            &prefix,
            &mut buffers.output,
        );
        writer.write_all(&buffers.output)?;
        total += match_count;
    }

    Ok(total)
}

fn run_files_parallel<W: Write>(
    writer: &mut W,
    file_paths: Vec<PathBuf>,
    compiled: &CompiledRegex,
    config: SearchConfig,
) -> Result<usize> {
    let thread_count = config.threads.min(file_paths.len()).max(1);
    let (job_tx, job_rx) = bounded::<FileJob>(JOB_CHANNEL_BOUND);
    let (result_tx, result_rx) = unbounded::<FileResult>();
    thread::scope(|scope| -> Result<usize> {
        for _ in 0..thread_count {
            let result_tx = result_tx.clone();
            let job_rx = job_rx.clone();
            scope.spawn(move || worker_loop(job_rx, result_tx, compiled, config));
        }
        drop(result_tx);

        for (sequence_no, path) in file_paths.into_iter().enumerate() {
            job_tx.send(FileJob { sequence_no, path })?;
        }
        drop(job_tx);

        let mut pending = BTreeMap::new();
        let mut next_sequence = 0usize;
        let mut total = 0usize;

        for result in result_rx.iter() {
            pending.insert(result.sequence_no, result);

            while let Some(result) = pending.remove(&next_sequence) {
                writer.write_all(&result.rendered_output)?;
                total += result.match_count;
                if let Some(error) = result.error {
                    return Err(anyhow::anyhow!(error));
                }
                next_sequence += 1;
            }
        }

        Ok(total)
    })
}

fn worker_loop(
    job_rx: Receiver<FileJob>,
    result_tx: crossbeam_channel::Sender<FileResult>,
    compiled: &CompiledRegex,
    config: SearchConfig,
) {
    let mut buffers = WorkerBuffers::default();
    for job in job_rx.iter() {
        buffers.reset_output();
        let result = match load_file_content(&job.path, &mut buffers.text).and_then(|content| {
            search_text_content(
                content.as_str(&job.path)?,
                compiled,
                &config,
                &display_prefix(&job.path, config.show_prefix),
                &mut buffers.output,
            )
            .pipe(Ok)
        }) {
            Ok(match_count) => FileResult {
                sequence_no: job.sequence_no,
                match_count,
                rendered_output: std::mem::take(&mut buffers.output),
                error: None,
            },
            Err(err) => FileResult {
                sequence_no: job.sequence_no,
                match_count: 0,
                rendered_output: Vec::new(),
                error: Some(format!("{err:#}")),
            },
        };
        if result_tx.send(result).is_err() {
            break;
        }
    }
}

fn load_file_content(path: &Path, reusable_text: &mut String) -> Result<FileContent> {
    let file = File::open(path)?;
    let metadata = file.metadata()?;

    if should_mmap(&metadata) {
        if let Ok(mmap) = unsafe { Mmap::map(&file) } {
            return Ok(FileContent::Mapped(mmap));
        }
    }

    reusable_text.clear();
    let mut file = file;
    file.read_to_string(reusable_text)?;
    Ok(FileContent::Owned(reusable_text.clone()))
}

fn should_mmap(metadata: &std::fs::Metadata) -> bool {
    metadata.len() >= MMAP_THRESHOLD_BYTES && cfg!(not(target_os = "macos"))
}

fn display_prefix(path: &Path, show_prefix: bool) -> String {
    if !show_prefix {
        String::new()
    } else {
        format!("{}:", path.to_string_lossy())
    }
}

#[doc(hidden)]
pub fn search_text_content(
    input: &str,
    pattern: &CompiledRegex,
    config: &SearchConfig,
    prefix: &str,
    output: &mut Vec<u8>,
) -> usize {
    if pattern.supports_candidate_lines() {
        search_with_candidates(input, pattern, config, prefix, output)
    } else {
        search_line_by_line(input, pattern, config, prefix, output)
    }
}

#[doc(hidden)]
pub fn search_with_candidates(
    input: &str,
    pattern: &CompiledRegex,
    config: &SearchConfig,
    prefix: &str,
    output: &mut Vec<u8>,
) -> usize {
    let mut match_count = 0;
    let mut search_from = 0usize;

    while let Some(candidate) = pattern.find_candidate_line(input, search_from) {
        let position = match candidate {
            LineCandidate::Confirmed(pos) | LineCandidate::Candidate(pos) => pos,
        };
        let (line_start, line_end) = line_bounds(input, position);
        let line = strip_line_terminator(&input[line_start..line_end]);
        let matches = find_all_regex_spans_compiled(line, pattern);
        if matches.is_empty() {
            search_from = line_end;
            continue;
        }
        match_count += write_line_matches(output, line, prefix, config, &matches);
        search_from = line_end;
    }

    match_count
}

#[doc(hidden)]
pub fn search_line_by_line(
    input: &str,
    pattern: &CompiledRegex,
    config: &SearchConfig,
    prefix: &str,
    output: &mut Vec<u8>,
) -> usize {
    let mut match_count = 0;
    for line in input.lines() {
        let matches = find_all_regex_spans_compiled(line, pattern);
        if matches.is_empty() {
            continue;
        }
        match_count += write_line_matches(output, line, prefix, config, &matches);
    }
    match_count
}

fn write_line_matches(
    output: &mut Vec<u8>,
    line: &str,
    prefix: &str,
    config: &SearchConfig,
    matches: &[RegexMatch],
) -> usize {
    if config.only_matching {
        for matched in matches {
            output.extend_from_slice(prefix.as_bytes());
            output.extend_from_slice(&line.as_bytes()[matched.start..matched.end]);
            output.push(b'\n');
        }
        matches.len()
    } else {
        write_rendered_line(output, line, prefix, config.use_color, matches)
            .expect("writing to vec should not fail");
        1
    }
}

fn write_rendered_line<W: Write>(
    writer: &mut W,
    line: &str,
    prefix: &str,
    use_color: bool,
    matches: &[RegexMatch],
) -> io::Result<()> {
    if matches.is_empty() || !use_color {
        writer.write_all(prefix.as_bytes())?;
        writer.write_all(line.as_bytes())?;
        writer.write_all(b"\n")?;
        return Ok(());
    }

    writer.write_all(prefix.as_bytes())?;

    let mut last = 0;
    for matched in matches {
        writer.write_all(&line.as_bytes()[last..matched.start])?;
        writer.write_all(ANSI_BOLD_RED)?;
        writer.write_all(&line.as_bytes()[matched.start..matched.end])?;
        writer.write_all(ANSI_RESET)?;
        last = matched.end;
    }
    writer.write_all(&line.as_bytes()[last..])?;
    writer.write_all(b"\n")
}

fn line_bounds(input: &str, position: usize) -> (usize, usize) {
    let line_start = input[..position].rfind('\n').map_or(0, |idx| idx + 1);
    let line_end = input[position..]
        .find('\n')
        .map_or(input.len(), |idx| position + idx + 1);
    (line_start, line_end)
}

fn strip_line_terminator(line: &str) -> &str {
    line.strip_suffix('\n')
        .and_then(|line| line.strip_suffix('\r').or(Some(line)))
        .unwrap_or(line)
}

#[doc(hidden)]
pub fn collect_files(inputs: &[PathBuf], recursive: bool) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for input in inputs {
        let meta = input.metadata()?;
        if meta.is_file() {
            files.push(input.clone());
        } else if meta.is_dir() {
            if !recursive {
                bail!("{}: Is a directory", input.to_string_lossy());
            }
            files.extend(collect_dir(input)?);
        }
    }
    Ok(files)
}

fn collect_dir(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut entries = dir.read_dir()?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        let path = entry.path();
        let meta = entry.metadata()?;
        if meta.is_dir() {
            files.extend(collect_dir(&path)?);
        } else if meta.is_file() {
            files.push(path);
        }
    }
    Ok(files)
}

trait Pipe: Sized {
    fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
        f(self)
    }
}

impl<T> Pipe for T {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile_regex;
    use std::{
        fs,
        sync::atomic::{AtomicUsize, Ordering},
    };

    static TEST_ID: AtomicUsize = AtomicUsize::new(0);

    fn temp_path(name: &str) -> PathBuf {
        let id = TEST_ID.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("grep-rust-test-{}-{id}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn writes_single_ansi_highlight() {
        let mut output = Vec::new();
        write_rendered_line(
            &mut output,
            "I have 3 apples",
            "",
            true,
            &[RegexMatch { start: 7, end: 8 }],
        )
        .unwrap();

        assert_eq!(
            String::from_utf8(output).unwrap(),
            "I have \x1b[1;31m3\x1b[0m apples\n"
        );
    }

    #[test]
    fn writes_multiple_ansi_highlights() {
        let mut output = Vec::new();
        write_rendered_line(
            &mut output,
            "a1b2c3",
            "",
            true,
            &[
                RegexMatch { start: 1, end: 2 },
                RegexMatch { start: 3, end: 4 },
                RegexMatch { start: 5, end: 6 },
            ],
        )
        .unwrap();

        assert_eq!(
            String::from_utf8(output).unwrap(),
            "a\x1b[1;31m1\x1b[0mb\x1b[1;31m2\x1b[0mc\x1b[1;31m3\x1b[0m\n"
        );
    }

    #[test]
    fn recursive_search_requires_flag_for_directories() {
        let dir = temp_path("dir");
        fs::create_dir_all(&dir).unwrap();
        let err = collect_files(&[dir], false).unwrap_err();
        assert!(err.to_string().contains("Is a directory"));
    }

    #[test]
    fn only_matching_parallel_output_stays_in_input_order() {
        let file1 = temp_path("first.txt");
        let file2 = temp_path("second.txt");
        fs::write(&file1, "foo\n").unwrap();
        fs::write(&file2, "foo\n").unwrap();

        let files = vec![file1.clone(), file2.clone()];
        let compiled = compile_regex("foo");
        let config = SearchConfig {
            only_matching: true,
            use_color: false,
            show_prefix: true,
            threads: 2,
        };

        let mut output = Vec::new();
        let count = run_files_parallel(&mut output, files, &compiled, config).unwrap();

        assert_eq!(count, 2);
        assert_eq!(
            String::from_utf8(output).unwrap(),
            format!(
                "{}:foo\n{}:foo\n",
                file1.to_string_lossy(),
                file2.to_string_lossy()
            )
        );
    }

    #[test]
    fn candidate_line_search_matches_line_by_line_for_literal_and_automata() {
        let config = SearchConfig {
            only_matching: false,
            use_color: false,
            show_prefix: false,
            threads: 1,
        };
        let input = "ordinary line\nmessage=matched_line_42\nanother line\n";

        for regex in [
            "matched_line_",
            r"message=(matched_line|ordinary_line)_[0123456789]+",
        ] {
            let compiled = compile_regex(regex);
            let mut candidate_output = Vec::new();
            let mut line_output = Vec::new();

            let candidate_count =
                search_with_candidates(input, &compiled, &config, "", &mut candidate_output);
            let line_count = search_line_by_line(input, &compiled, &config, "", &mut line_output);

            assert_eq!(candidate_count, line_count);
            assert_eq!(candidate_output, line_output);
        }
    }
}

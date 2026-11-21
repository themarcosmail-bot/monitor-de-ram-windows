use std::{
    hint::black_box,
    io::{self, BufRead, BufReader, Write},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    time::Instant,
};

use crate::windows::{MIB, Process};

const MEMORY_BYTES: usize = 128 * MIB as usize;
const PAGE_BYTES: usize = 4096;
const ROUNDS: usize = 5;

#[test]
#[ignore = "manual A/B benchmark using a dedicated child"]
fn resume_after_trim() {
    let mut passive = Vec::new();
    let mut trimmed = Vec::new();
    for round in 0..ROUNDS {
        for trim in [round % 2 == 1, round % 2 == 0] {
            let mut worker = Worker::start();
            let process = Process::open(worker.child.id(), trim).expect("open only the test child");
            let before = process.resident_bytes().expect("initial resident memory");
            assert!(
                before > 96 * MIB,
                "test child was paged out before measurement"
            );
            if trim {
                assert!(process.trim(), "trim only the test child");
            }
            let after = process
                .resident_bytes()
                .expect("resident memory before resuming");
            let elapsed = worker.measure_resume();
            let sample = (before.saturating_sub(after) / MIB, elapsed);
            if trim {
                trimmed.push(sample);
            } else {
                passive.push(sample);
            }
        }
    }
    let (passive_mib, passive_us) = medians(&passive);
    let (trimmed_mib, trimmed_us) = medians(&trimmed);
    println!(
        "RAM_AB passive_mib={passive_mib} passive_us={passive_us} trimmed_mib={trimmed_mib} trimmed_us={trimmed_us}"
    );
    assert!(
        trimmed_mib >= 64,
        "the Windows API did not reclaim the expected resident pages"
    );
}

fn medians(samples: &[(u64, u128)]) -> (u64, u128) {
    let mut memory: Vec<_> = samples.iter().map(|sample| sample.0).collect();
    let mut times: Vec<_> = samples.iter().map(|sample| sample.1).collect();
    memory.sort_unstable();
    times.sort_unstable();
    (memory[memory.len() / 2], times[times.len() / 2])
}

struct Worker {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}

impl Worker {
    fn start() -> Self {
        let mut child = Command::new(std::env::current_exe().expect("test executable"))
            .args(["--ignored", "--exact", "benchmark::worker", "--nocapture"])
            .env("MONITOR_RAM_BENCHMARK_CHILD", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("start dedicated test child");
        let input = child.stdin.take().expect("child stdin");
        let output = BufReader::new(child.stdout.take().expect("child stdout"));
        let mut worker = Self {
            child,
            input,
            output,
        };
        worker.response("FIXTURE_READY");
        worker
    }

    fn measure_resume(&mut self) -> u128 {
        writeln!(self.input, "resume").expect("resume test child");
        self.input.flush().expect("flush test command");
        let elapsed = self
            .response("FIXTURE_US ")
            .parse::<u128>()
            .expect("elapsed microseconds");
        assert!(self.child.wait().expect("wait for test child").success());
        elapsed
    }

    fn response(&mut self, marker: &str) -> String {
        for line in (&mut self.output).lines() {
            let line = line.expect("read test child");
            if let Some((_, payload)) = line.split_once(marker) {
                return payload.trim().to_string();
            }
        }
        panic!("test child did not send {marker}");
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
#[ignore = "child fixture, started by resume_after_trim"]
fn worker() {
    if std::env::var_os("MONITOR_RAM_BENCHMARK_CHILD").is_none() {
        return;
    }
    let mut pages = vec![0u8; MEMORY_BYTES];
    for (index, page) in pages.chunks_exact_mut(PAGE_BYTES).enumerate() {
        page[0] = black_box(index as u8);
    }
    let mut input = String::new();
    println!("FIXTURE_READY");
    io::stdout().flush().expect("mark the fixture ready");
    io::stdin()
        .read_line(&mut input)
        .expect("read fixture command");
    assert_eq!(input.trim(), "resume");

    let start = Instant::now();
    let checksum = pages
        .chunks_exact(PAGE_BYTES)
        .fold(0u64, |sum, page| sum + u64::from(black_box(page[0])));
    let elapsed = start.elapsed();
    assert!(checksum > 0);
    println!("FIXTURE_US {}", elapsed.as_micros());
    io::stdout().flush().expect("return timing");
}

use std::time::{Duration, Instant};

pub struct Benchmark {
    name: String,
    start_time: Instant,
    iterations: usize,
    total_duration: Duration,
}

impl Benchmark {
    pub fn new(name: String) -> Self {
        Self {
            name,
            start_time: Instant::now(),
            iterations: 0,
            total_duration: Duration::ZERO,
        }
    }

    pub fn start(&mut self) {
        self.start_time = Instant::now();
    }

    pub fn stop(&mut self) {
        self.total_duration += self.start_time.elapsed();
        self.iterations += 1;
    }

    pub fn lap(&mut self) {
        self.stop();
        self.start();
    }

    pub fn results(&self) -> BenchmarkResult {
        BenchmarkResult {
            name: self.name.clone(),
            iterations: self.iterations,
            total_duration: self.total_duration,
            avg_duration: if self.iterations > 0 {
                self.total_duration / (self.iterations as u32)
            } else {
                Duration::ZERO
            },
        }
    }
}

pub struct BenchmarkResult {
    name: String,
    iterations: usize,
    total_duration: Duration,
    avg_duration: Duration,
}

impl BenchmarkResult {
    pub fn report(&self) -> String {
        format!(
            "{}: {} iterations, {} total, {:.2}µs avg per iteration",
            self.name,
            self.iterations,
            format_duration(self.total_duration),
            self.avg_duration.as_secs_f64() * 1_000_000.0
        )
    }

    pub fn throughput(&self, bytes: usize) -> String {
        if self.iterations > 0 && self.total_duration > Duration::ZERO {
            let total_bytes = bytes * self.iterations;
            let throughput = total_bytes as f64 / self.total_duration.as_secs_f64();
            format!("{:.2} bytes/sec", throughput)
        } else {
            "N/A".to_string()
        }
    }
}

fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    let millis = d.subsec_millis();
    let micros = d.subsec_micros();
    if secs > 0 {
        format!("{}.{:03}s", secs, millis)
    } else if millis > 0 {
        format!("{}.{:03}ms", millis, micros)
    } else {
        format!("{}µs", micros)
    }
}

pub fn run_screen_benchmark(iterations: usize) -> BenchmarkResult {
    use crate::screen::Screen;

    let mut benchmark = Benchmark::new("screen_update".to_string());

    for _ in 0..iterations {
        benchmark.start();
        let mut screen = Screen::new(80, 24);
        for i in 0..100 {
            screen.process(format!("{}", i).as_bytes());
        }
        screen.process(b"\x1b[2J");
        benchmark.stop();
    }

    benchmark.results()
}

pub fn run_parser_benchmark(iterations: usize) -> BenchmarkResult {
    let mut benchmark = Benchmark::new("ansi_parser".to_string());
    let test_data = b"\x1b[31mred\x1b[0m\x1b[1mbold\x1b[22m";

    for _ in 0..iterations {
        benchmark.start();
        let mut parser = crate::ansi::AnsiParser::new();
        parser.parse(test_data);
        benchmark.stop();
    }

    benchmark.results()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_benchmark_creation() {
        let benchmark = Benchmark::new("test".to_string());
        assert_eq!(benchmark.name, "test");
        assert_eq!(benchmark.iterations, 0);
    }

    #[test]
    fn test_benchmark_lap() {
        let mut benchmark = Benchmark::new("test".to_string());
        benchmark.start();
        std::thread::sleep(Duration::from_millis(10));
        benchmark.lap();
        assert_eq!(benchmark.iterations, 1);
    }

    #[test]
    fn test_benchmark_results() {
        let mut benchmark = Benchmark::new("test".to_string());
        benchmark.start();
        std::thread::sleep(Duration::from_millis(1));
        benchmark.stop();
        let results = benchmark.results();
        assert!(results.iterations >= 1);
        assert!(results.total_duration > Duration::ZERO);
    }
}

use std::fmt::Display;
use std::ops::{Add, AddAssign};

use indexmap::IndexMap;

#[derive(Debug, Clone, Default)]
pub struct Profiler {
    total_time: f64,
    total_frames: u64,
    timings: IndexMap<String, Vec<f64>>,
}

#[derive(Debug, Clone, Copy)]
pub struct Timing {
    pub avg: f64,
    pub variance: f64,
    pub max: f64,
    pub min: f64,
}
impl Default for Timing {
    fn default() -> Self {
        Self {
            avg: 0.0,
            variance: 0.0,
            max: 0.0,
            min: 0.0,
        }
    }
}
impl Timing {
    pub fn deviation(&self) -> f64 {
        self.variance.sqrt()
    }
}
impl Display for Timing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:.3}ms (±{:.3}ms, {:.3}ms ~ {:.3}ms)",
            self.avg,
            self.deviation(),
            self.min,
            self.max
        )
    }
}
impl Add<Timing> for Timing {
    type Output = Timing;
    fn add(self, rhs: Timing) -> Self::Output {
        Timing {
            avg: self.avg + rhs.avg,
            variance: self.variance + rhs.variance,
            max: self.max + rhs.max,
            min: self.min + rhs.min,
        }
    }
}
impl AddAssign<Timing> for Timing {
    fn add_assign(&mut self, rhs: Timing) {
        *self = *self + rhs;
    }
}

impl Profiler {
    pub fn time(&self) -> f64 {
        self.total_time
    }
    pub fn frames(&self) -> u64 {
        self.total_frames
    }
    pub fn new() -> Self {
        Self::default()
    }
    pub fn record(&mut self, timings: Vec<(String, f32)>) {
        let mut total_time = 0.0;
        for (name, time) in timings {
            self.timings.entry(name).or_default().push(time as f64);
            total_time += time as f64;
        }
        self.total_time += total_time;
        self.total_frames += 1;
    }
    pub fn reset(&mut self) {
        self.total_time = 0.0;
        self.total_frames = 0;
        for (_, times) in self.timings.iter_mut() {
            times.clear();
        }
    }
    pub fn timings(&self) -> Vec<(String, Timing)> {
        self.timings
            .iter()
            .map(|(name, timings)| {
                let avg = timings.iter().sum::<f64>() / timings.len() as f64;
                let variance =
                    timings.iter().map(|t| (t - avg).powi(2)).sum::<f64>() / timings.len() as f64;
                let max = timings.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                let min = timings.iter().copied().fold(f64::INFINITY, f64::min);
                (
                    name.clone(),
                    Timing {
                        avg,
                        variance,
                        max,
                        min,
                    },
                )
            })
            .collect()
    }

    pub fn report(&self, sections: &[&str], display_subsections: bool) -> String {
        let mut report = String::new();
        report.push_str(&format!(
            "\nFrame Time: {:.3}ms ({} frames)",
            self.total_time / self.total_frames as f64,
            self.total_frames
        ));
        let timings = self.timings();
        for section in sections {
            let mut total_timing = Timing::default();
            for (name, timing) in &timings {
                if name.starts_with(section) {
                    total_timing += *timing;
                }
            }
            report.push_str(&format!("\n{section}: {total_timing}"));
            if display_subsections {
                for (name, timing) in &timings {
                    if let Some(name) = name.strip_prefix(section) {
                        let name = name.trim_start();
                        if name.is_empty() {
                            continue;
                        }
                        report.push_str(&format!("\n  {name}: {timing}"));
                    }
                }
            }
        }
        report
    }
    pub fn print(&self, sections: &[&str], display_subsections: bool) {
        println!("{}", self.report(sections, display_subsections));
    }
}

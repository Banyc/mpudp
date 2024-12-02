use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use primitive::sync::mutex::SpinMutex;
use rand::Rng;

const EPSILON_LATENCY: Duration = Duration::from_millis(1);
const EXPLORE_PROB: f64 = 0.3;

pub type Stats = Arc<[SpinMutex<Stat>]>;
pub fn new_stats(conns: usize) -> Stats {
    let now = Instant::now();
    let mut stats = vec![];
    for _ in 0..conns {
        let stat = Stat::new(now);
        stats.push(SpinMutex::new(stat));
    }
    stats.into()
}

#[derive(Debug, Clone, Copy)]
pub struct Stat {
    last_sent_start: Instant,
    last_recv: Instant,
    prev_latency: Duration,
}
impl Stat {
    pub fn new(now: Instant) -> Self {
        Self {
            last_sent_start: now,
            last_recv: now,
            prev_latency: Duration::ZERO,
        }
    }
    pub fn latency(&self, now: Instant) -> Duration {
        if self.last_sent_start < self.last_recv {
            self.last_recv - self.last_sent_start
        } else {
            self.prev_latency.max(now - self.last_sent_start)
        }
    }
    pub fn sent(&mut self, now: Instant) {
        if self.last_recv < self.last_sent_start {
            return;
        }
        self.prev_latency = self.last_recv - self.last_sent_start;
        self.last_sent_start = now;
    }
    pub fn recv(&mut self, now: Instant) {
        self.last_recv = now;
    }
}

#[derive(Debug, Clone)]
pub struct Rank {
    values: Vec<f64>,
    last_update: Instant,
}
impl Rank {
    pub fn new(values: usize, now: Instant) -> Self {
        let values = (0..values).map(|_| 1. / values as f64).collect();
        Self {
            values,
            last_update: now,
        }
    }
    pub fn update_rank<'a>(&mut self, stats: impl Iterator<Item = &'a Stat> + Clone, now: Instant) {
        rank(stats, &mut self.values, now);
    }
    pub fn choose_exploit(&self) -> Option<usize> {
        if self.values.is_empty() {
            return None;
        }
        let mut rng = rand::thread_rng();
        let mut remaining = rng.gen_range(0. ..1.);
        for (i, &value) in self.values.iter().enumerate() {
            if remaining < value {
                return Some(i);
            }
            remaining -= value;
        }
        Some(self.values.len() - 1)
    }
    pub fn choose_explore(&self, except: usize) -> Option<usize> {
        assert!(except < self.values.len());
        let mut rng = rand::thread_rng();
        let p = rng.gen_range(0. ..=1.);
        if p < EXPLORE_PROB {
            return None;
        }
        let next = rng.gen_range(0..self.values.len() - 1);
        Some(if next < except { next } else { next + 1 })
    }
}

fn rank<'a>(stats: impl Iterator<Item = &'a Stat> + Clone, out: &mut Vec<f64>, now: Instant) {
    let mut latency_sum = Duration::ZERO;
    for stat in stats.clone() {
        let latency = stat.latency(now).max(EPSILON_LATENCY);
        latency_sum += latency;
    }
    out.clear();
    for stat in stats {
        let latency = stat.latency(now).max(EPSILON_LATENCY);
        let weight = latency.as_secs_f64() / latency_sum.as_secs_f64();
        out.push(weight);
    }
}

//! Parsing of remote `/proc` + `df` snapshots into footer stats (session_stats command).

use serde::Serialize;

/// One snapshot of remote counters. Percent fields are computed at parse time; the
/// cumulative byte/jiffy counters need a previous sample to become rates.
#[derive(Clone, Copy, Default, PartialEq)]
pub struct Sample {
    pub cpu: Option<(u64, u64)>, // (total, idle+iowait) jiffies from /proc/stat
    pub rx: Option<u64>,         // received bytes, every interface but `lo`
    pub tx: Option<u64>,
    pub mem_pct: Option<u8>,
    pub disk_pct: Option<u8>,
}

/// What the footer shows. `None` renders as `--` (non-Linux host, or first sample).
#[derive(Serialize, Clone, Default, Debug, PartialEq)]
pub struct Stats {
    pub cpu: Option<f64>,
    pub mem: Option<u8>,
    pub disk: Option<u8>,
    pub rx_kbps: Option<f64>,
    pub tx_kbps: Option<f64>,
}

fn kb(value: &str) -> Option<u64> {
    value.trim().split_whitespace().next()?.parse().ok()
}

pub fn parse(text: &str) -> Sample {
    let mut s = Sample::default();
    let (mut total_kb, mut avail_kb) = (None, None);
    let mut disk = None;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("cpu ") {
            let v: Vec<u64> = rest.split_whitespace().filter_map(|f| f.parse().ok()).collect();
            if !v.is_empty() {
                // /proc/stat: user nice system idle iowait irq softirq steal ...
                let idle =
                    v.get(3).copied().unwrap_or(0).saturating_add(v.get(4).copied().unwrap_or(0));
                s.cpu = Some((v.iter().sum(), idle));
            }
        } else if let Some((k, r)) = line.split_once(':') {
            match k.trim() {
                "MemTotal" => total_kb = kb(r),
                "MemAvailable" => avail_kb = kb(r),
                // /proc/net/dev: `  eth0: rx_bytes ... (8 fields) tx_bytes ... (8 fields)`
                _ => {
                    let f: Vec<u64> = r.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                    let name = k.trim();
                    if f.len() >= 16 && !name.is_empty() && name != "lo" {
                        s.rx = Some(s.rx.unwrap_or(0).wrapping_add(f[0]));
                        s.tx = Some(s.tx.unwrap_or(0).wrapping_add(f[8]));
                    }
                }
            }
        } else {
            // df -k: Filesystem 1K-blocks Used Available Use% Mounted (header has `Use%`)
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.len() >= 5 {
                if let Some(p) = f[4].trim().strip_suffix('%').and_then(|x| x.parse::<u8>().ok()) {
                    disk = Some(p);
                }
            }
        }
    }
    if let (Some(t), Some(a)) = (total_kb, avail_kb) {
        if t > 0 {
            s.mem_pct = Some((((t - a.min(t)) * 100) / t) as u8);
        }
    }
    s.disk_pct = disk;
    s
}

pub fn diff(prev: Option<Sample>, now: Sample, secs: f64) -> Stats {
    let cpu = match (prev.and_then(|p| p.cpu), now.cpu) {
        (Some((pt, pi)), Some((nt, ni))) if nt > pt && secs > 0.0 => {
            Some((100.0 * (1.0 - (ni - pi.min(ni)) as f64 / (nt - pt) as f64)).clamp(0.0, 100.0))
        }
        _ => None,
    };
    let rate = |prev: Option<u64>, cur: Option<u64>| match (prev, cur) {
        (Some(p), Some(n)) if n >= p && secs > 0.0 => Some((n - p) as f64 / secs / 1024.0),
        _ => None,
    };
    Stats {
        cpu,
        mem: now.mem_pct,
        disk: now.disk_pct,
        rx_kbps: rate(prev.and_then(|p| p.rx), now.rx),
        tx_kbps: rate(prev.and_then(|p| p.tx), now.tx),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SNAPSHOT: &str = "cpu  100 0 100 700 100 0 0 0 0 0\ncpu0 50 0 50 900 0 0 0 0\n\
        MemTotal:       8000 kB\nMemAvailable:   2000 kB\nMemFoo: 1 kB\n\
        Inter-|   Receive                        |  Transmit\n\
        footer garbage without colon\n\
          eth0: 2048 1 2 3 4 5 6 7 1024 1 2 3 4 5 6 7\n\
             lo: 99 1 2 3 4 5 6 7 99 1 2 3 4 5 6 7\n\
        Filesystem     1K-blocks    Used Available Use% Mounted on\n\
        /dev/vda1       10000000 4000000   6000000  40% /";

    #[test]
    fn parses_proc_and_df() {
        let s = parse(SNAPSHOT);
        // total = 1000, idle = 700 + 100
        assert_eq!(s.cpu, Some((1000, 800)));
        assert_eq!(s.rx, Some(2048)); // `lo` excluded
        assert_eq!(s.tx, Some(1024));
        assert_eq!(s.mem_pct, Some(75)); // (8000-2000)/8000
        assert_eq!(s.disk_pct, Some(40));
    }

    #[test]
    fn first_sample_has_rates_only_after_delta() {
        let now = parse(SNAPSHOT);
        let first = diff(None, now, 3.0);
        assert_eq!(first.cpu, None);
        assert_eq!(first.rx_kbps, None);
        assert_eq!(first.mem, Some(75));

        let later = SNAPSHOT.replace("cpu  100", "cpu  200"); // +100 total, idle unchanged
        let s2 = parse(&later);
        let stats = diff(Some(now), s2, 10.0);
        assert_eq!(stats.cpu, Some(100.0)); // all 100 new jiffies busy
        assert_eq!(stats.rx_kbps, Some(0.0)); // same byte counters → 0 KB/s
    }

    #[test]
    fn missing_files_yield_all_none() {
        let s = parse("nothing useful\n");
        assert_eq!(diff(None, s, 1.0), Stats::default());
    }
}

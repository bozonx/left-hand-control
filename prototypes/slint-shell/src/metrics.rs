use std::{fs::File, io::Write, time::Instant};

pub struct Metrics {
    file: File,
    start: Instant,
    next: u64,
    active: std::collections::HashMap<&'static str, Trial>,
}

struct Trial {
    id: u64,
    start: Instant,
    source: &'static str,
    seen: Vec<&'static str>,
}

impl Metrics {
    pub fn new(start: Instant) -> std::io::Result<Self> {
        let path = std::env::var("SLINT_SHELL_METRICS").unwrap_or("slint-shell.csv".into());
        let mut file = File::create(path)?;
        writeln!(file, "trial,window,source,event,elapsed_ms,process_ms")?;
        Ok(Self {
            file,
            start,
            next: 0,
            active: Default::default(),
        })
    }

    pub fn ready(&mut self, window: &str) {
        let ms = self.start.elapsed().as_secs_f64() * 1000.;
        log::info!("{window} ready at {ms:.3} ms");
        self.write(format!("0,{window},startup,ready,{ms:.3},{ms:.3}"));
    }

    fn write(&mut self, line: String) {
        if let Err(error) = writeln!(self.file, "{line}").and_then(|_| self.file.flush()) {
            log::error!("metrics: {error}");
        }
    }

    pub fn begin(&mut self, window: &'static str, source: &'static str, start: Instant) {
        self.next += 1;
        self.active.insert(
            window,
            Trial {
                id: self.next,
                start,
                source,
                seen: vec![],
            },
        );
        self.mark(window, "t0_trigger");
        self.mark(window, "t1_on_ui_thread");
    }

    pub fn mark(&mut self, window: &'static str, event: &'static str) {
        let Some(trial) = self.active.get_mut(window) else {
            return;
        };
        if trial.seen.contains(&event) {
            return;
        }
        trial.seen.push(event);
        let ms = if event == "t0_trigger" {
            0.
        } else {
            trial.start.elapsed().as_secs_f64() * 1000.
        };
        let process_ms = self.start.elapsed().as_secs_f64() * 1000.;
        let line = format!(
            "{},{window},{},{event},{ms:.3},{:.3}",
            trial.id, trial.source, process_ms
        );
        self.write(line);
    }

    pub fn trial(&self, window: &str) -> Option<u64> {
        self.active.get(window).map(|trial| trial.id)
    }

    pub fn end(&mut self, window: &'static str) {
        self.mark(window, "hidden");
        self.active.remove(window);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forwarded_trigger_before_worker_start_does_not_underflow() {
        let path = std::env::temp_dir().join(format!(
            "slint-metrics-forwarded-{}.csv",
            std::process::id()
        ));
        let start = Instant::now();
        let mut metrics = Metrics {
            file: File::create(&path).unwrap(),
            start,
            next: 0,
            active: Default::default(),
        };
        metrics.begin(
            "emoji",
            "evdev",
            start - std::time::Duration::from_millis(10),
        );
        metrics.mark("emoji", "t4_focused");
        drop(metrics);
        let rows = std::fs::read_to_string(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        let focus = rows
            .lines()
            .find(|line| line.contains(",t4_focused,"))
            .unwrap();
        let fields: Vec<_> = focus.split(',').collect();
        assert!(fields[4].parse::<f64>().unwrap() >= 10.0);
        assert!(fields[5].parse::<f64>().unwrap() >= 0.0);
    }

    #[test]
    fn repeated_frames_and_late_focus_do_not_contaminate_next_trial() {
        let path = std::env::temp_dir().join(format!("slint-metrics-{}.csv", std::process::id()));
        let start = Instant::now();
        let mut metrics = Metrics {
            file: File::create(&path).unwrap(),
            start,
            next: 0,
            active: Default::default(),
        };
        metrics.begin("emoji", "ipc", start);
        metrics.mark("emoji", "t3_first_frame");
        metrics.mark("emoji", "t3_first_frame");
        metrics.end("emoji");
        metrics.mark("emoji", "t4_focused");
        metrics.begin("emoji", "evdev", Instant::now());
        metrics.mark("emoji", "t4_focused");
        metrics.end("emoji");
        drop(metrics);
        let rows = std::fs::read_to_string(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(rows.matches("t3_first_frame").count(), 1);
        assert_eq!(rows.matches("t4_focused").count(), 1);
        assert!(rows.contains("2,emoji,evdev,t4_focused,"));
        assert!(!rows.contains("1,emoji,ipc,t4_focused,"));
        assert_eq!(rows.matches(",hidden,").count(), 2);
    }
}

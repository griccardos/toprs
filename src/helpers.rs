use formato::Formato;

pub fn nice_size(val: u64) -> String {
    nice_size_ops(val, false)
}

pub fn nice_size_thousands(val: u64) -> String {
    nice_size_ops(val, true)
}

fn nice_size_ops(val: u64, include_thousands: bool) -> String {
    let format = if include_thousands { "#,###.0" } else { "#.0" };
    if val == 0 {
        "".to_string()
    } else if val < 5000 {
        format!("{val}B")
    } else if val < 500 * 1024 {
        format!("{}K", (val as f64 / 1024.).formato(format))
    } else if val < 50000 * 1024 * 1024 {
        format!("{}M", (val as f64 / 1024. / 1024.).formato(format))
    } else {
        format!("{}G", (val as f64 / 1024. / 1024. / 1024.).formato(format))
    }
}

pub fn nice_size_g(val: u64) -> String {
    format!("{:.1}G", val as f64 / 1024. / 1024. / 1024.)
}

pub fn nice_size_g_thousands(val: u64) -> String {
    (val as f64 / 1024. / 1024. / 1024.).formato("#,###.0G")
}

pub fn nice_time(time_seconds: u64) -> String {
    let mut secs = time_seconds;
    if secs < 60 {
        format!("{time_seconds}s")
    } else if secs < 60 * 60 {
        let m = secs / 60;
        secs -= m * 60;
        let s = secs;
        format!("{m:02}m {s:02}s")
    } else if secs < 60 * 60 * 24 {
        let h = secs / (60 * 60);
        secs -= h * 60 * 60;
        let m = secs / 60;
        secs -= m * 60;
        let s = secs;
        format!("{h}h {m:>2}m {s:>2}s ")
    } else {
        let d = secs / (60 * 60 * 24);
        secs -= d * 60 * 60 * 24;
        let h = secs / (60 * 60);
        secs -= h * 60 * 60;
        let m = secs / 60;
        secs -= m * 60;
        let s = secs;
        format!("{d:>2}d {h:>2}h {m:>2}m {s:>2}s ")
    }
}

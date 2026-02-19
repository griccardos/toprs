use formato::Formato;

pub trait IntoF64 {
    fn into_f64<T>(&self) -> f64;
}
impl IntoF64 for f64 {
    fn into_f64<T>(&self) -> f64 {
        *self
    }
}
impl IntoF64 for u64 {
    fn into_f64<T>(&self) -> f64 {
        *self as f64
    }
}

pub fn nice_size<T: IntoF64>(val: T) -> String {
    nice_size_ops(val, false)
}

#[cfg(feature = "gui")]
pub fn nice_size_thousands<T: IntoF64>(val: T) -> String {
    nice_size_ops(val, true)
}

fn nice_size_ops<T: IntoF64>(val: T, include_thousands: bool) -> String {
    let format = if include_thousands { "#,###.0" } else { "#.0" };
    let val: f64 = val.into_f64::<T>();
    if val == 0.0 {
        "".to_string()
    } else if val < 5000.0 {
        format!("{}B", (val as f64).formato(format))
    } else if val < 500.0 * 1024.0 {
        format!("{}K", (val as f64 / 1024.).formato(format))
    } else if val < 50000.0 * 1024.0 * 1024.0 {
        format!("{}M", (val as f64 / 1024. / 1024.).formato(format))
    } else {
        format!("{}G", (val as f64 / 1024. / 1024. / 1024.).formato(format))
    }
}

pub fn nice_size_g(val: u64) -> String {
    format!("{:.1}G", val as f64 / 1024. / 1024. / 1024.)
}

#[cfg(feature = "gui")]
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

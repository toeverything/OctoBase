#![allow(unused)]

use std::collections::HashMap;

fn process_duration(duration: &str) -> (f64, f64) {
    let dur_split: Vec<String> = duration.split("±").map(String::from).collect();
    let units = dur_split[1].chars().skip_while(|c| c.is_digit(10)).collect::<String>();
    let dur_secs = convert_dur_to_seconds(dur_split[0].parse::<f64>().unwrap(), &units);
    let error_secs = convert_dur_to_seconds(
        dur_split[1]
            .chars()
            .take_while(|c| c.is_digit(10))
            .collect::<String>()
            .parse::<f64>()
            .unwrap(),
        &units,
    );
    (dur_secs, error_secs)
}

fn convert_dur_to_seconds(dur: f64, units: &str) -> f64 {
    let factors = [
        ("s", 1.0),
        ("ms", 1.0 / 1000.0),
        ("µs", 1.0 / 1_000_000.0),
        ("ns", 1.0 / 1_000_000_000.0),
    ]
    .iter()
    .cloned()
    .collect::<HashMap<_, _>>();
    dur * factors.get(units).unwrap_or(&1.0)
}

#[allow(dead_code)]
fn is_significant(changes_dur: f64, changes_err: f64, base_dur: f64, base_err: f64) -> bool {
    if changes_dur < base_dur {
        changes_dur + changes_err < base_dur || base_dur - base_err > changes_dur
    } else {
        changes_dur - changes_err > base_dur || base_dur + base_err < changes_dur
    }
}

#[cfg(feature = "bench")]
fn convert_to_markdown() -> impl Iterator<Item = String> {
    let re = regex::Regex::new(r"\s{2,}").unwrap();
    std::io::stdin()
        .lines()
        .skip(2)
        .map(|l| l.ok())
        .map(move |row| {
            if let Some(row) = row {
                let columns = re.split(&row).collect::<Vec<_>>();
                let name = columns.get(0)?;
                let base_duration = columns.get(2)?;
                let changes_duration = columns.get(5)?;
                Some((
                    name.to_string(),
                    base_duration.to_string(),
                    changes_duration.to_string(),
                ))
            } else {
                None
            }
        })
        .flatten()
        .map(|(name, base_duration, changes_duration)| {
            let mut difference = "N/A".to_string();
            let base_undefined = base_duration == "?";
            let changes_undefined = changes_duration == "?";

            if !base_undefined && !changes_undefined {
                let (base_dur_secs, base_err_secs) = process_duration(&base_duration);
                let (changes_dur_secs, changes_err_secs) = process_duration(&changes_duration);

                let diff = -(1.0 - changes_dur_secs / base_dur_secs) * 100.0;
                difference = format!("{:+.2}%", diff);

                if is_significant(changes_dur_secs, changes_err_secs, base_dur_secs, base_err_secs) {
                    difference = format!("**{}**", difference);
                }
            }

            format!(
                "| {} | {} | {} | {} |",
                name.replace("|", "\\|"),
                if base_undefined { "N/A" } else { &base_duration },
                if changes_undefined { "N/A" } else { &changes_duration },
                difference
            )
        })
}

fn main() {
    let platform = std::env::args().into_iter().skip(1).next().unwrap();

    #[cfg(feature = "bench")]
    for ret in [
        &format!("## Benchmark for {platform}"),
        "<details>",
        "  <summary>Click to view benchmark</summary>",
        "",
        "| Test | Base | PR | % |",
        "| --- | --- | --- | --- |",
    ]
    .into_iter()
    .map(|s| s.to_owned())
    .chain(convert_to_markdown())
    .chain(["</details>".to_owned()])
    {
        println!("{}", ret);
    }
}

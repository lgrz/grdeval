extern crate getopts;

use std::process;
use std::error::Error;
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::collections::HashMap;
use std::cmp::Ordering;
use getopts::Options;

pub fn run(config: Config) -> Result<(), Box<Error>> {
    let mut ideal_gain: HashMap<String, f64> = HashMap::new();
    let mut judgments: HashMap<String, i32> = HashMap::new();
    let mut gain: Vec<i32> = Vec::new();
    let mut qrels: Vec<QrelEntry> = Vec::new();
    let mut run: Vec<TrecEntry> = Vec::new();
    let mut curr_topic = String::new();
    let mut max_judgment: i32 = 0;

    let f = File::open(config.qrelfile).expect("file not found");
    for line in BufReader::new(f).lines() {
        let s = line.unwrap();
        let qentry = QrelEntry::new(s.split(" ").collect());
        if qentry.relevance > max_judgment {
            max_judgment = qentry.relevance;
        }
        if qentry.relevance > 0 {
            qrels.push(qentry);
        }
    }

    let f = File::open(config.runfile).expect("file not found");
    for line in BufReader::new(f).lines() {
        let s = line.unwrap();
        run.push(TrecEntry::new(s.split(" ").collect()));
    }

    // sort qrels by topic then judgement
    qrels.sort_by(|a, b| a.topic.cmp(&b.topic)
                  .then_with(|| b.relevance.cmp(&a.relevance)));
    for qentry in qrels.iter() {
        if curr_topic != qentry.topic {
            if curr_topic != "" {
                ideal_gain.insert(curr_topic.to_string(), dcg(config.cutoff, &gain));
                gain.clear();
            }
            curr_topic = qentry.topic.clone();
        }
        if qentry.relevance < 0 {
            continue;
        }
        judgments.insert(format!("{}:{}", qentry.topic, qentry.docid), qentry.relevance);
        gain.push(qentry.relevance);
    }
    if curr_topic != "" {
        ideal_gain.insert(curr_topic.to_string(), dcg(config.cutoff, &gain));
        gain.clear();
    }

    // sort run file by topic then score then docid
    run.sort_by(|a, b| a.topic.cmp(&b.topic)
                .then_with(|| b.partial_cmp(&a).unwrap())
                .then_with(|| b.docid.cmp(&a.docid)));

    let mut ndcg_total = 0.;
    let mut err_total = 0.;
    let mut topics = 0.;
    let runid = &run[0].runid;
    curr_topic.clear();
    gain.clear();

    println!("runid,topic,ndcg@{},err@{}", config.cutoff, config.cutoff);
    for entry in run.iter() {
        if curr_topic != entry.topic {
            if curr_topic != "" {
                if ideal_gain.contains_key(&curr_topic) {
                let curr_ideal = match ideal_gain.get(&curr_topic) {
                    Some(&x) => x,
                    None => 0.,
                };
                let mut curr_ndcg = 0.;
                if curr_ideal > 0. {
                    curr_ndcg = dcg(config.cutoff, &gain) / curr_ideal;
                }
                ndcg_total += curr_ndcg;
                let curr_err = err(config.cutoff, &gain, max_judgment as u32);
                err_total += curr_err;
                topics += 1.;
                println!("{},{},{:.*},{:.*}", runid, curr_topic, 5, curr_ndcg, 5, curr_err);
                }
            }
            curr_topic = entry.topic.clone();
            gain.clear();
        }
        let mut j = match judgments.get(&format!("{}:{}", entry.topic, entry.docid)) {
            Some(&x) => x,
            _ => 0,
        };
        if j < 0 {
            j = 0;
        }
        gain.push(j);
    }
    if curr_topic != "" {
        let curr_ideal = match ideal_gain.get(&curr_topic) {
            Some(&x) => x,
            None => 0.,
        };
        let mut curr_ndcg = 0.;
        if curr_ideal > 0. {
            curr_ndcg = dcg(config.cutoff, &gain) / curr_ideal;
        }
        ndcg_total += curr_ndcg;
        let curr_err = err(config.cutoff, &gain, max_judgment as u32);
        err_total += curr_err;
        topics += 1.;
        println!("{},{},{:.*},{:.*}", runid, curr_topic, 5, curr_ndcg, 5, curr_err);
    }
    println!("{},{},{:.*},{:.*}", runid, "amean", 5, ndcg_total / topics, 5, err_total / topics);

    Ok(())
}

pub struct Config {
    qrelfile: String,
    runfile: String,
    cutoff: usize,
}

impl Config {
    pub fn new(args: &[String]) -> Result<Config, &'static str> {
        let program = args[0].clone();
        let mut opts = Options::new();
        opts.optopt("k", "", "depth of ranking to evaluate to", "");
        opts.optflag("h", "help", "print this help menu");

        let matches = match opts.parse(&args[1..]) {
            Ok(m) => { m }
            _ => {
                Config::usage(&program, opts);
                process::exit(1);
            }
        };

        if matches.opt_present("h") {
            Config::usage(&program, opts);
            process::exit(0);
        }

        if matches.free.len() < 2 {
            Config::usage(&program, opts);
            process::exit(1);
        }

        let cutoff = match matches.opt_str("k") {
            Some(x) => x.parse::<usize>().unwrap(),
            None => 20,
        };

        let qrelfile = matches.free[0].clone();
        let runfile = matches.free[1].clone();

        Ok(Config { qrelfile, runfile, cutoff })
    }

    pub fn usage(name: &str, opts: Options) {
        let brief = format!("Usage: {} [options] <qrels> <file>", name);
        print!("{}", opts.usage(&brief));
    }
}

#[allow(unused_variables)]
pub struct QrelEntry {
    topic: String,
    docid: String,
    relevance: i32,
}

impl QrelEntry {
    pub fn new(vec: Vec<&str>) -> QrelEntry {
        if vec.len() != 4 {
            panic!("qrel fields not 4");
        }

        let topic = vec[0].to_string();
        let docid = vec[2].to_string();
        let relevance = vec[3].parse::<i32>().unwrap();

        QrelEntry { topic, docid, relevance }
    }
}

pub struct TrecEntry {
    topic: String,
    docid: String,
    score: f64,
    runid: String,
}

impl TrecEntry {
    pub fn new(vec: Vec<&str>) -> TrecEntry {
        if vec.len() != 6 {
            panic!("run fields not 6");
        }

        let topic = vec[0].to_string();
        let docid = vec[2].to_string();
        let score = vec[4].parse::<f64>().unwrap();
        let runid = vec[5].to_string();

        TrecEntry {
            topic,
            docid,
            score,
            runid,
        }
    }
}

impl PartialOrd for TrecEntry {
    fn partial_cmp(&self, other: &TrecEntry) -> Option<Ordering> {
        self.score.partial_cmp(&other.score)
    }
}

impl PartialEq for TrecEntry {
    fn eq(&self, other: &TrecEntry) -> bool {
        self.score == other.score
    }
}

pub fn dcg(k: usize, gain: &Vec<i32>) -> f64 {
    if gain.len() < 1 {
        return 0.
    }

    let mut score = 0.;
    for (i, n) in gain.iter().take(k).enumerate() {
        let n = *n as u32;
        let i = i as f64;
        score += (2_i32.pow(n) - 1) as f64 / (i + 2.).log(2.0)
    }

    score
}

pub fn err(k: usize, gain: &Vec<i32>, max_judgment: u32) -> f64 {
    let mut score = 0.;
    let mut decay = 1.0;

    for (i, n) in gain.iter().take(k).enumerate() {
        let n = *n as u32;
        let i = i as f64;
        let r = (2_i32.pow(n) - 1) as f64 / (2_i32.pow(max_judgment)) as f64;
        score += r as f64 * decay / (i + 1.);
        decay *= (1. - r) as f64;
    }

    score
}

#[cfg(test)]
mod test {
    use super::*;
    use std::f64;

    const dummy_max_judgement: u32 = 3;

    #[test]
    fn dcg_when_k_is_zero() {
        assert_eq!(0., dcg(0, &vec![1]));
    }

    #[test]
    fn dcg_when_gain_is_empty() {
        assert_eq!(0., dcg(10, &vec![]));
    }

    #[test]
    fn dcg_calculate() {
        let dcg_expected = 9.392789260714371;
        let abs_diff = (dcg(3, &vec![3, 2, 1]) - dcg_expected).abs();

        assert!(abs_diff <= f64::EPSILON);
    }

    #[test]
    fn dcg_calculate_less_than_cutoff() {
        let dcg_expected = 9.392789260714371;
        let abs_diff = (dcg(5, &vec![3, 2, 1]) - dcg_expected).abs();

        assert!(abs_diff <= f64::EPSILON);
    }

    #[test]
    fn dcg_calculate_greater_than_cutoff() {
        let dcg_expected = 7.;
        let abs_diff = (dcg(1, &vec![3, 2, 1]) - dcg_expected).abs();

        assert!(abs_diff <= f64::EPSILON);
    }

    #[test]
    fn err_when_k_is_zero() {
        assert_eq!(0., err(0, &vec![1], dummy_max_judgement));
    }

    #[test]
    fn err_when_gain_is_empty() {
        assert_eq!(0., err(10, &vec![], dummy_max_judgement));
    }

    #[test]
    fn err_calculate() {
        let err_expected = 0.9016927083333334;
        let abs_diff = (err(3, &vec![3, 2, 1], dummy_max_judgement) - err_expected).abs();

        assert!(abs_diff <= f64::EPSILON);
    }

    #[test]
    fn err_calculate_less_than_cutoff() {
        let err_expected = 0.9016927083333334;
        let abs_diff = (err(5, &vec![3, 2, 1], dummy_max_judgement) - err_expected).abs();

        assert!(abs_diff <= f64::EPSILON);
    }

    #[test]
    fn err_calculate_greater_than_cutoff() {
        let err_expected = 0.875;
        let abs_diff = (err(1, &vec![3, 2, 1], dummy_max_judgement) - err_expected).abs();

        assert!(abs_diff <= f64::EPSILON);
    }
}

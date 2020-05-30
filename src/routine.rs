use crate::workout::FlatStatus;
use serde::Deserialize;
use serde_dhall::from_str;
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Deserialize, Debug)]
struct SimpleWork {
    duration: u32,
    name: String,
}

#[derive(Clone, Deserialize, Debug)]
enum Work {
    Simple(SimpleWork),
    Ref(String),
}

#[derive(Deserialize, Debug)]
struct SetRepeat {
    repeats: usize,
    work: Work,
}
#[derive(Deserialize, Debug)]
enum Set {
    Set(Vec<Work>),
    Repeat(SetRepeat),
}

#[derive(Deserialize, Debug)]
struct SetWithRests {
    rest: u32,
    work: Set,
}

#[derive(Deserialize, Debug)]
pub struct Routine {
    definitions: HashMap<String, SetWithRests>,
    top: String,
}
impl Routine {
    pub fn to_full_workout(&self) -> Vec<FlatStatus> {
        self.to_workout(&self.top)
    }
    pub fn to_workout(&self, top: &str) -> Vec<FlatStatus> {
        let mut current = vec![top];
        let mut ans = Vec::new();
        while !current.is_empty() {
            let c = current.pop().unwrap();
            let lu = self.definitions.get(c).unwrap();
            let work_list = match &lu.work {
                Set::Set(list) => Cow::Borrowed(list),
                Set::Repeat(sr) => Cow::Owned(vec![sr.work.clone(); sr.repeats]),
            };
            for (ix, w) in work_list.iter().enumerate() {
                if ix > 0 {
                    ans.push(FlatStatus {
                        name: "rest".to_owned(),
                        duration: lu.rest,
                        this_rep: (ix as u32) + 1,
                        total_reps: work_list.len() as u32,
                        absolute_start_time: 0,
                    })
                }
                match w {
                    Work::Simple(sw) => ans.push(FlatStatus {
                        name: sw.name.to_owned(),
                        duration: sw.duration,
                        this_rep: (ix as u32) + 1,
                        total_reps: work_list.len() as u32,
                        absolute_start_time: 0,
                    }),
                    Work::Ref(n) => {
                        let mut v = self.to_workout(n);
                        ans.append(&mut v);
                    }
                }
            }
        }
        ans
    }
}

pub fn mk7min() -> Routine {
    let data = TYPES.to_string() + SEVEN;
    serde_dhall::from_str(&data).parse().unwrap()
}

const TYPES: &'static str = include_str!("../data/types.dhall");
const SEVEN: &'static str = include_str!("../data/7min.dhall");

#[cfg(test)]
mod test {
    use super::*;
    const JOE: &'static str = include_str!("../data/joe.dhall");
    const TRIVIAL: &'static str = include_str!("../data/trivial.dhall");
    #[test]
    pub fn simple_parse() {
        let data = TYPES.to_string() + TRIVIAL;
        let parsed = serde_dhall::from_str(&data)
            .parse::<Routine>()
            .unwrap_or_else(|e| panic!("{}", e));
        assert_eq!(parsed.definitions.len(), 0);
        assert_eq!(parsed.top, "TEST");
    }
    #[test]
    pub fn joe_parse() {
        let data = TYPES.to_string() + JOE;
        let parsed = serde_dhall::from_str(&data)
            .parse::<Routine>()
            .unwrap_or_else(|e| panic!("{}", e));
        dbg!(&parsed);
        println!("{:?}", &parsed);
        assert_eq!(parsed.definitions.len(), 3);
        assert_eq!(parsed.top, "all");
    }
}

use csv;
use serde::{Serialize,Deserialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct WorkoutStep {
    pub name: String,
    pub duration: Option<u32>,
    pub rest_before: u32,
    pub rest_after: u32,
}

impl WorkoutStep {
    pub fn from_csv(csv_str: &str) -> Result<Vec<Self>, csv::Error> {
        let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .trim(csv::Trim::All)
        .from_reader(csv_str.as_bytes());
        let mut ans = Vec::new();
        for r in reader.deserialize() {
            let r: Self = r?;
            ans.push(r);
        }
        Ok(ans)
    }
    pub fn to_csv(list: &[Self]) -> Result<String,csv::Error> {
        let mut writer = csv::Writer::from_writer(vec![]);
        for l in list {
            writer.serialize(l)?;
        }
        let data = String::from_utf8(writer.into_inner().unwrap()).unwrap();
        Ok(data)
    }
    pub fn total_duration(&self) -> Option<u32> {
        self.duration.map(|d| d + self.rest_before + self.rest_after)
    }
}

pub const SEVEN: &'static str = include_str!("../data/7min.txt");
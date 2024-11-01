#![no_std]

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone, defmt::Format)]
pub enum AdcSamplingPeriod {
    CYCLES1_5,
    CYCLES7_5,
    CYCLES13_5,
    CYCLES28_5,
    CYCLES41_5,
    CYCLES55_5,
    CYCLES71_5,
    CYCLES239_5,
}

impl AdcSamplingPeriod {
    pub fn to_seconds(&self) -> f64 {
        use AdcSamplingPeriod::*;
        let cycles = match self {
            CYCLES1_5 => 1.5,
            CYCLES7_5 => 7.5,
            CYCLES13_5 => 13.5,
            CYCLES28_5 => 28.5,
            CYCLES41_5 => 41.5,
            CYCLES55_5 => 55.5,
            CYCLES71_5 => 71.5,
            CYCLES239_5 => 239.5,
        };

        let adc_frequency = 12_000_000.;
        let adc_sample_overhead_cycles = 12.5; // see reference manual section 11.6
        (cycles + adc_sample_overhead_cycles) / adc_frequency
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone, defmt::Format)]
#[allow(non_snake_case)]
pub enum Command {
    SetFrequency {
        frequency_kHz: f64,
        adc_sampling_period: AdcSamplingPeriod,
    },
    Record,
}

impl Command {
    pub fn serialize<'a>(&self, buf: &'a mut [u8]) -> Result<&'a mut [u8], postcard::Error> {
        postcard::to_slice(self, buf)
    }

    pub fn deserialize(bs: &[u8]) -> Option<Self> {
        postcard::from_bytes(bs).ok()
    }
}

#![no_std]
#![no_main]
#![feature(impl_trait_in_assoc_type)]

use core::fmt::Write;
use embassy_executor::Spawner;
use embassy_time::Timer;
use esp_backtrace as _;
use esp_hal::{
    analog::adc::{Adc, AdcPin, Attenuation, CalibrationAccess},
    gpio::{AnyPin, Input, Level, Output, Pull},
    peripherals::ADC1,
    prelude::*,
};
use esp_println::logger::init_logger;
use heapless::String;

const MEASURE_PIN: u8 = 0;

const RES_TABLE: [u32; 5] = [100, 1000, 10000, 100000, 1000000];
const CORRECTION_FACTOR: [f32; 5] = [1.25, 1.23, 1.0, 1.65, 1.07];

#[allow(non_snake_case)]
struct Meter<'d> {
    sample_index: usize,
    samples: [f32; 10],
    channel: u8,
    res: u32,
    range_100_ohm: Output<'d, AnyPin>,
    range_1k_ohm: Output<'d, AnyPin>,
    range_10k_ohm: Output<'d, AnyPin>,
    range_100k_ohm: Output<'d, AnyPin>,
    range_1Mm_ohm: Output<'d, AnyPin>,
}

#[allow(non_snake_case)]
impl<'d> Meter<'d> {
    fn new(
        range_100_ohm: Output<'d, AnyPin>,
        range_1k_ohm: Output<'d, AnyPin>,
        range_10k_ohm: Output<'d, AnyPin>,
        range_100k_ohm: Output<'d, AnyPin>,
        range_1Mm_ohm: Output<'d, AnyPin>,
    ) -> Self {
        Self {
            sample_index: 0,
            samples: [0.0; 10],
            channel: 4,
            res: RES_TABLE[4],
            range_100_ohm,
            range_1k_ohm,
            range_10k_ohm,
            range_100k_ohm,
            range_1Mm_ohm,
        }
    }

    async fn read(&mut self, volt_image: u16) -> Option<String<32>> {
        if volt_image >= 2200 && self.channel < 4 {
            self.channel += 1;
            self.set_channel(self.channel);
            Timer::after_millis(50).await;
            return None;
        }

        if volt_image <= 330 && self.channel > 0 {
            self.channel -= 1;
            self.set_channel(self.channel);
            Timer::after_millis(50).await;
            return None;
        }

        let value =
            (volt_image as f32 * self.res as f32) / (4096 as f32 - volt_image as f32) as f32;
        let mut buffer = String::<32>::new();

        self.samples[self.sample_index] = value * CORRECTION_FACTOR[self.channel as usize];
        self.sample_index = (self.sample_index + 1) % 10;

        let value = self.samples.iter().sum::<f32>() / 10.0;

        if volt_image < 3600 {
            if value < 1000.0 {
                // For values less than 1000 Ohms
                let integer_part = value as u16;
                let fractional_part = ((value * 10.0) as u16) % 10;

                // Format into the buffer
                write!(&mut buffer, "{:03}.{} Ohm", integer_part, fractional_part).unwrap();
            } else if value < 10_000.0 {
                // For values between 1 KΩ and 10 KΩ
                let integer_part = (value / 1000.0) as u16;
                let fractional_part = (value as u16) % 1000;

                write!(&mut buffer, "{}.{:03} KOhm", integer_part, fractional_part).unwrap();
            } else if value < 100_000.0 {
                // For values between 10 KΩ and 100 KΩ
                let integer_part = (value / 1000.0) as u16;
                let fractional_part = ((value / 100.0) as u16) % 10;

                write!(
                    &mut buffer,
                    "{:02}.{:02} KOhm",
                    integer_part, fractional_part
                )
                .unwrap();
            } else if value < 1_000_000.0 {
                // For values between 100 KΩ and 1 MΩ
                let integer_part = (value / 1000.0) as u16;
                let fractional_part = ((value / 100.0) as u16) % 10;

                write!(&mut buffer, "{:03}.{} KOhm", integer_part, fractional_part).unwrap();
            } else {
                // For values 1 MΩ and above
                let integer_part = (value / 1_000_000.0) as u16;
                let fractional_part = ((value / 1000.0) as u16) % 1000;

                write!(&mut buffer, "{}.{:03} MOhm", integer_part, fractional_part).unwrap();
            }
        } else {
            write!(&mut buffer, "<infinity>").unwrap();
        }

        Some(buffer)
    }

    fn set_channel(&mut self, channel: u8) {
        log::trace!("set_channel: {}", channel);
        self.channel = channel;
        self.sample_index = 0;
        self.samples = [0.0; 10];
        match channel {
            0 => {
                self.range_100_ohm.set_low();
                self.range_1k_ohm.set_high();
                self.range_10k_ohm.set_high();
                self.range_100k_ohm.set_high();
                self.range_1Mm_ohm.set_high();
            }
            1 => {
                self.range_100_ohm.set_high();
                self.range_1k_ohm.set_low();
                self.range_10k_ohm.set_high();
                self.range_100k_ohm.set_high();
                self.range_1Mm_ohm.set_high();
            }
            2 => {
                self.range_100_ohm.set_high();
                self.range_1k_ohm.set_high();
                self.range_10k_ohm.set_low();
                self.range_100k_ohm.set_high();
                self.range_1Mm_ohm.set_high();
            }
            3 => {
                self.range_100_ohm.set_high();
                self.range_1k_ohm.set_high();
                self.range_10k_ohm.set_high();
                self.range_100k_ohm.set_low();
                self.range_1Mm_ohm.set_high();
            }
            4 => {
                self.range_100_ohm.set_high();
                self.range_1k_ohm.set_high();
                self.range_10k_ohm.set_high();
                self.range_100k_ohm.set_high();
                self.range_1Mm_ohm.set_low();
            }
            _ => {}
        }

        self.res = RES_TABLE[channel as usize];
    }
}

#[main]
async fn main(_spawner: Spawner) {
    init_logger(log::LevelFilter::Trace);

    let peripherals = esp_hal::init({
        let mut config = esp_hal::Config::default();
        config.cpu_clock = CpuClock::max();
        config
    });

    let timg0 = esp_hal::timer::timg::TimerGroup::new(peripherals.TIMG0);
    esp_hal_embassy::init(timg0.timer0);
    log::info!("Embassy initialized");

    let range_100_ohm = esp_hal::gpio::Output::new(peripherals.GPIO1, Level::Low);
    let range_1k_ohm = esp_hal::gpio::Output::new(peripherals.GPIO2, Level::Low);
    let range_10k_ohm = esp_hal::gpio::Output::new(peripherals.GPIO21, Level::Low);
    let range_100k_ohm = esp_hal::gpio::Output::new(peripherals.GPIO22, Level::Low);
    #[allow(non_snake_case)]
    let range_1Mm_ohm = esp_hal::gpio::Output::new(peripherals.GPIO23, Level::Low);

    let mut meter = Meter::new(
        range_100_ohm,
        range_1k_ohm,
        range_10k_ohm,
        range_100k_ohm,
        range_1Mm_ohm,
    );

    type AdcCal = esp_hal::analog::adc::AdcCalCurve<ADC1>;
    let mut adc_config = esp_hal::analog::adc::AdcConfig::default();
    let mut adc_pin = adc_config.enable_pin_with_cal::<_, AdcCal>(
        peripherals.GPIO0,
        esp_hal::analog::adc::Attenuation::Attenuation11dB,
    );
    let mut adc = esp_hal::analog::adc::Adc::new(peripherals.ADC1, adc_config);

    meter.set_channel(4);

    loop {
        let voltage = adc.read_oneshot(&mut adc_pin).unwrap();
        let result = meter.read(voltage).await;
        log::trace!(
            "result: {:?}, corrected: {}, channel: {}, voltage: {}, sample_index: {}",
            result,
            ((voltage as f32 * meter.res as f32) / (4096 as f32 - voltage as f32)) * 1.25,
            meter.channel,
            voltage,
            meter.sample_index
        );

        Timer::after_millis(100).await;
    }
}

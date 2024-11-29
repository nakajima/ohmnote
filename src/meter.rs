use core::fmt::Write;
use embassy_time::Timer;
use esp_hal::gpio::{AnyPin, Output};
use heapless::String;

const RES_TABLE: [u32; 5] = [100, 1000, 10000, 100000, 1700000];

#[allow(non_snake_case)]
pub struct Meter<'d> {
    pub sample_index: usize,
    pub samples: [f32; 50],
    pub channel: u8,
    pub res: u32,
    pub range_100_ohm: Output<'d, AnyPin>,
    pub range_1k_ohm: Output<'d, AnyPin>,
    pub range_10k_ohm: Output<'d, AnyPin>,
    pub range_100k_ohm: Output<'d, AnyPin>,
    pub range_1Mm_ohm: Output<'d, AnyPin>,
}

#[allow(non_snake_case)]
impl<'d> Meter<'d> {
    pub fn new(
        range_100_ohm: Output<'d, AnyPin>,
        range_1k_ohm: Output<'d, AnyPin>,
        range_10k_ohm: Output<'d, AnyPin>,
        range_100k_ohm: Output<'d, AnyPin>,
        range_1Mm_ohm: Output<'d, AnyPin>,
    ) -> Self {
        Self {
            sample_index: 0,
            samples: [0.0; 50],
            channel: 4,
            res: RES_TABLE[4],
            range_100_ohm,
            range_1k_ohm,
            range_10k_ohm,
            range_100k_ohm,
            range_1Mm_ohm,
        }
    }

    pub async fn is_connected(&mut self, volt_image: u16) -> bool {
        let value = (volt_image as f32 * self.res as f32) / (3250_f32 - volt_image as f32);

        value < 5_000_000.0
    }

    pub async fn read(&mut self, volt_image: u16) -> Option<String<32>> {
        let mut buffer = String::<32>::new();

        if volt_image >= 1650 && self.channel < 4 {
            self.channel += 1;
            self.set_channel(self.channel);
            Timer::after_millis(5).await;
            return None;
        }

        if volt_image <= 270 && self.channel > 0 {
            self.channel -= 1;
            self.set_channel(self.channel);
            Timer::after_millis(5).await;
            return None;
        }

        let value =
            (volt_image as f32 * self.res as f32) / (3250 as f32 - volt_image as f32) as f32;

        self.samples[self.sample_index] = value;
        self.sample_index = (self.sample_index + 1) % self.samples.len();

        let value = self.samples.iter().sum::<f32>() / self.samples.len() as f32;

        // log::info!("volt_image: {}, value: {}", volt_image, value);

        if volt_image < 2700 {
            if value < 1000.0 {
                // For values less than 1000 Ohms
                let integer_part = value as u16;

                // Format into the buffer
                write!(&mut buffer, "{}", integer_part).unwrap();
            } else if value < 10_000.0 {
                // For values between 1 KΩ and 10 KΩ
                let integer_part = (value / 1000.0) as u16;
                let fractional_part = (value as u16) % 1000;

                write!(&mut buffer, "{}.{:03}K", integer_part, fractional_part).unwrap();
            } else if value < 100_000.0 {
                // For values between 10 KΩ an                                               d 100 KΩ
                let integer_part = (value / 1000.0) as u16;
                let fractional_part = ((value / 100.0) as u16) % 10;

                write!(&mut buffer, "{:02}.{:02}K", integer_part, fractional_part).unwrap();
            } else if value < 1_000_000.0 {
                // For values between 100 KΩ and 1 MΩ
                let integer_part = (value / 1000.0) as u16;
                let fractional_part = ((value / 100.0) as u16) % 10;

                write!(&mut buffer, "{:03}.{}K", integer_part, fractional_part).unwrap();
            } else {
                // For values 1 MΩ and above
                let integer_part = (value / 1_000_000.0) as u16;
                let fractional_part = ((value / 1000.0) as u16) % 1000;

                if integer_part >= 4 {
                    return None;
                } else {
                    write!(&mut buffer, "{}.{:03}M", integer_part, fractional_part).unwrap();
                }
            }
        } else {
            return None;
        }

        Some(buffer)
    }

    pub fn set_channel(&mut self, channel: u8) {
        log::trace!("set_channel: {}", channel);
        self.channel = channel;
        self.sample_index = 0;
        self.samples = [0.0; 50];
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

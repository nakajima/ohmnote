#![no_std]
#![no_main]
#![feature(impl_trait_in_assoc_type)]

use embassy_executor::Spawner;
use embassy_time::Timer;
use esp_backtrace as _;
use esp_hal::{
    gpio::{Level, Output},
    i2c::{self, master::I2c},
    peripherals::ADC1,
    prelude::*,
};
use esp_println::logger::init_logger;
use ohmnote::{
    display::{init_display, READING},
    meter::Meter,
};

#[main]
async fn main(spawner: Spawner) {
    init_logger(log::LevelFilter::Trace);

    let peripherals = esp_hal::init({
        let mut config = esp_hal::Config::default();
        config.cpu_clock = CpuClock::max();
        config
    });

    let timg0 = esp_hal::timer::timg::TimerGroup::new(peripherals.TIMG0);
    esp_hal_embassy::init(timg0.timer0);
    log::info!("Embassy initialized");

    let scl = peripherals.GPIO20;
    let sda = peripherals.GPIO19;
    let rst = peripherals.GPIO18;

    let mut config = i2c::master::Config::default();
    config.frequency = 1_000.kHz();
    let i2c = I2c::new(peripherals.I2C0, config)
        .with_scl(scl)
        .with_sda(sda)
        .into_async();

    // Reset display
    let mut rst = Output::new(rst, Level::High);
    rst.set_high();
    Timer::after_millis(10).await;
    rst.set_low();
    Timer::after_millis(10).await;
    rst.set_high();
    Timer::after_millis(100).await;

    spawner.spawn(init_display(i2c)).unwrap();

    let range_100_ohm = esp_hal::gpio::Output::new(peripherals.GPIO1, Level::Low);
    let range_1k_ohm = esp_hal::gpio::Output::new(peripherals.GPIO2, Level::Low);
    let range_10k_ohm = esp_hal::gpio::Output::new(peripherals.GPIO21, Level::Low);
    let range_100k_ohm = esp_hal::gpio::Output::new(peripherals.GPIO22, Level::Low);
    #[allow(non_snake_case)]
    let range_1m_ohm = esp_hal::gpio::Output::new(peripherals.GPIO23, Level::Low);

    let mut meter = Meter::new(
        range_100_ohm,
        range_1k_ohm,
        range_10k_ohm,
        range_100k_ohm,
        range_1m_ohm,
    );

    type AdcCal = esp_hal::analog::adc::AdcCalBasic<ADC1>;
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

        READING.sender().send(result.clone());
        // log::trace!(
        //     "result: {:?}, channel: {}, voltage: {}, sample_index: {}",
        //     result,
        //     meter.channel,
        //     voltage,
        //     meter.sample_index
        // );
        Timer::after_millis(10).await;
    }
}

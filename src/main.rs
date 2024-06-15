use anyhow::{Error, Result};
use esp_idf_hal::delay::BLOCK;
use esp_idf_hal::io::Read;
use log::Log;
use std::str;
use esp_idf_hal::ledc::*;
use esp_idf_hal::units::Hertz;
use esp_idf_hal::spi::config::BitOrder;
use esp_idf_hal::spi::config::Duplex;
use esp_idf_hal::spi::config::Polarity;
use esp_idf_hal::spi::config::Mode;
use esp_idf_hal::spi::config::Phase;
use esp_idf_hal::spi::Operation;
use esp_idf_hal::delay::FreeRtos;
use esp_idf_hal::spi::{Spi, SpiDeviceDriver, SpiDriver, SpiDriverConfig, SPI2};
use esp_idf_hal::spi::config::{Config, DriverConfig};
use esp_idf_hal::gpio::{Gpio18, Gpio19, Gpio5,Gpio23,Gpio22};
use embedded_svc::{http::Method, io::Write,http::server::Connection,http::server::*};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::{
        i2c::{I2cConfig, I2cDriver},
        prelude::*,
    },
    http::{client::EspHttpConnection, server::{Configuration, EspHttpServer,Response,Request}},
};
use std::{
     fmt::Debug, sync::{Arc, Mutex}, thread::sleep, time::Duration, io::{self,Write as IoWrite},fs::File
};
use wifi::wifi;
    //W25Q32FV SPI FLASH KOMUTLARI
    const READ_DATA : u8 = 0x03;
    const PAGE_PROGRAM : u8 = 0x02; //needs write_enable = 1
    const WRITE_ENABLE : u8 = 0x06;
    const WRITE_DISABLE : u8 = 0x04;
    const SECTOR_ERASE : u8 = 0x20; //needs write_enable = 1
    const CHIP_ERASE : u8 = 0x60;
    const ENABLE_RESET : u8 = 0x66;
    const RESET : u8 = 0x99; //takes 30 microseconds
    const READ_UNIQUE_ID : u8 = 0x4b;
    const READ_STATUS_REGISTER_ONE : u8 = 0x05;
    const READ_STATUS_REGISTER_TWO : u8 = 0x35; 
    const READ_STATUS_REGISTER_THREE : u8 = 0x15;
    const READ_MANUFACTURER_DEVICE_ID : u8 = 0x90;
    const BLOCK_ERASE : u8 = 0xD8;
    fn clamp(value: usize, min: usize, max: usize) -> usize {
        if value < min {
            min
        } else if value > max {
            max
        } else {
            value
        }
    }
fn main() {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    let peripherals = Peripherals::take().unwrap();
    let sysloop = EspSystemEventLoop::take().expect("event loop error");

    //let vin = peripherals.pins.gpio22;
    let cs = peripherals.pins.gpio5;
    let miso = peripherals.pins.gpio19;
    let clk = peripherals.pins.gpio18;
    let mosi = peripherals.pins.gpio23;

    let spi_config = Config::new().baudrate(1.MHz().into())
    .bit_order(BitOrder::MsbFirst)
    .duplex(Duplex::Full)
    .data_mode(Mode{polarity: Polarity::IdleLow, phase: Phase::CaptureOnFirstTransition})
    .write_only(false);
    //let mut spi_config = Config::default();
    //spi_config.cs_active_high = false;

    let spi_driver = SpiDriver::new::<SPI2>(
        peripherals.spi2,
        clk,
        mosi,
        Some(miso),
        &SpiDriverConfig::new()
    ).expect("driver error");

    let spi_device = Arc::new(Mutex::new(SpiDeviceDriver::new(
        &spi_driver,
        Some(cs),
        &spi_config
    ).expect("spi driver error")));

    let mut read_data8: [u8;1] = [1];
    let mut read_data16: [u8;2] = [1,2];
    let mut read_data24: [u8;3] = [1,2,3];
    let mut read_data32: [u8;4] = [1,2,3,4];
    let mut read_data64: [u8;8] = [1,2,3,4,5,6,7,8];
    {
        let mut spi_device = spi_device.lock().unwrap();
        spi_device.transfer(&mut read_data64,&[READ_MANUFACTURER_DEVICE_ID,0x00,0x00,0x00]).expect("write error");
        log::info!("READ MANUFACTURER DEVICE ID: {read_data64:x?}");
        FreeRtos::delay_ms(10);
        spi_device.transfer(&mut read_data64,&[READ_DATA,0x3A,0xFF,0xFB]).expect("error");
        log::info!("READ DATA : {read_data64:x?}");
        spi_device.write(&[WRITE_ENABLE]).expect("error");
        spi_device.write(&[PAGE_PROGRAM,0x3A,0x00,0x00,0x6B,0x61,0x61,0x6E]).expect("error");
        spi_device.transfer(&mut read_data64,&[READ_DATA,0x3a,0x00,0x00]).expect("error");
        let name = String::from_utf8_lossy(&read_data64[4..8]);
        log::info!("READ OVERWRITTEN DATA : {name:x?}");
    }
    
    let mut ledChannel = LedcDriver::new(
        peripherals.ledc.channel0,
        LedcTimerDriver::new(
        peripherals.ledc.timer0,
        &config::TimerConfig::new().frequency(25.kHz().into())).expect("timerconfig error"),
        peripherals.pins.gpio2).expect("ledError");
    let _wifi = wifi(
        "FiberHGW_ZTQS7S_2.4GHz",
        "NTb7hpy9Hu",
        peripherals.modem,
        sysloop,
        ).expect("wifi error");
        let mut server = EspHttpServer::new(&Configuration::default()).expect("servermaking error");
    server.fn_handler("/", Method::Get, |request| -> Result<(),Error>{
        let mut response = request.into_ok_response().expect("response error");
        response.write_all(wifi::INDEX_HTML).expect("indexpageFunc");
        Ok(())
    }).expect("index handler error");
    server.fn_handler("/erase", Method::Post, |request| -> Result<(),Error>{
        let mut response = request.into_ok_response().expect("erase error");
        let mut spi_device = spi_device.lock().unwrap();
        spi_device.write(&[WRITE_ENABLE]).expect("write enable error");
        spi_device.write(&[CHIP_ERASE]).expect("block erase error");
        FreeRtos::delay_ms(200);
        response.write_all("veri silme basarili.".as_bytes()).expect("response write error");
        Ok(())
    }).expect("index handler error");
    //son 4 byte'ı oku
    server.fn_handler("/read", Method::Get, |request| -> Result<(),Error>{
        let mut response = request.into_ok_response().expect("read error");
        let mut spi_device = spi_device.lock().unwrap();
        let mut heap_vec: Vec<u8> = vec![0; 500];
        let mut code_vec =vec![READ_DATA,0x3A,0x00,0x00];
        //code_vec.append(&mut extension_vec);
        spi_device.transfer(&mut heap_vec,&code_vec).expect("error");
        //let name = String::(heap_vec[4..340].to_vec()).unwrap();
        //log::info!("{name}");
        //format!("{}{}{}",wifi::READ_HTML0,heap_vec,wifi::READ_HTML1);
        response.write_all(&[wifi::READ_HTML0,&heap_vec,wifi::READ_HTML1].concat()).expect("writing error");
        Ok(())
    }).expect("index handler error");
    //
    server.fn_handler("/write", Method::Get, |request| -> Result<(),Error>{
        let mut response = request.into_ok_response().expect("read error");
        response.write_all(wifi::LOAD_HTML).expect("loadpageFunc");
        Ok(())
    }).expect("index handler error");
    //flash'a yaz
    server.fn_handler("/upload", Method::Post, |mut request| -> Result<(),Error>{
        log::info!("UPLOAD EXECUTED");
        //let mut req_data_vec: Vec<u8> = vec![0; request.content_len().unwrap() as usize];
        let mut req_data_vec : [u8;500] = (0..500).map(|i| i as u8).collect::<Vec<u8>>().try_into().unwrap();
        request.read(&mut req_data_vec).expect("reading error");
        log::info!("{req_data_vec:x?}");

        let mut spi_device = spi_device.lock().unwrap();
        let mut counter = 0;
        for i in 0..256 {
            let mut code_vec = vec![PAGE_PROGRAM,0x3A,i as u8,0x00];
            spi_device.write(&[WRITE_ENABLE]).expect("write enable error");
            if counter+256 >= req_data_vec.len() {
                let mut counter_end = req_data_vec.len();
                code_vec.extend_from_slice(&req_data_vec[counter..counter_end]);
                spi_device.write(&code_vec).expect("page program error");
                log::info!("PAGEWRITE_LAST: {code_vec:x?}");
                break;
            }
            else {
                code_vec.extend_from_slice(&req_data_vec[counter..counter+256]);
                spi_device.write(&code_vec).expect("page program error");
                counter += 256;
                log::info!("PAGEWRITE: {code_vec:x?}");
                FreeRtos::delay_ms(10);
                }
        }
        log::info!("UPLOAD END");
        let mut response = request.into_ok_response().expect("read error");
        
        Ok(())
    }).expect("index handler error");
    ledChannel.set_duty(ledChannel.get_max_duty()/2).expect("led duty error");
    loop {
        sleep(Duration::from_millis(1000));
    }

}


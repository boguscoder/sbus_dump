#![no_std]
#![no_main]

mod usb;

use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::UART0;
use embassy_rp::uart::{Config, DataBits, InterruptHandler, Parity, StopBits};
use embassy_time::{with_timeout, Duration, Instant};
use panic_probe as _;

bind_interrupts!(struct Irqs {
    UART0_IRQ => InterruptHandler<UART0>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    spawner.must_spawn(usb::usb_setup(p.USB));

    // rc via SBUS
    let uart = p.UART0;
    let rx = p.PIN_29;
    let dma = p.DMA_CH2;

    let mut sbus_uart_config = Config::default();
    sbus_uart_config.baudrate = 100_000;
    sbus_uart_config.data_bits = DataBits::DataBits8;
    sbus_uart_config.stop_bits = StopBits::STOP2;
    sbus_uart_config.parity = Parity::ParityEven;
    sbus_uart_config.invert_rx = true;

    pub type UartRxSbusPeripheral =
        embassy_rp::uart::UartRx<'static, UART0, embassy_rp::uart::Async>;

    let mut uart_rx: UartRxSbusPeripheral =
        embassy_rp::uart::UartRx::new(uart, rx, Irqs, dma, sbus_uart_config);
    let mut read_buffer = [0u8; 25];
    let mut sbusparser = sbus::SBusPacketParser::new();
    let mut parse_time = Instant::now();
    let timeout = Duration::from_millis(200);

    loop {
        match with_timeout(timeout, uart_rx.read(&mut read_buffer)).await {
            Ok(Ok(_)) => {
                sbusparser.push_bytes(&read_buffer);
                // If packet was parsed with no failsafe, send it
                if let Some(packet) = sbusparser.try_parse() {
                    parse_time = Instant::now();
                    match packet.failsafe {
                        false => log::info!("{:?}", packet.channels),
                        true => log::error!("Failsafe"),
                    }

                // Else if packet was not parsed, check if it took too long
                } else if parse_time.elapsed() > timeout {
                    log::error!("Parse timeout");
                }
            }
            Ok(Err(e)) => log::error!("Serial read {:?}", e),
            Err(_) => log::error!("Serial timeout"),
        }
    }
}

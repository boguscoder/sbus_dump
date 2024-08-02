#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::UART1;
use embassy_rp::uart::{Config, DataBits, InterruptHandler, Parity, StopBits};
use embassy_time::{with_timeout, Duration, Instant};
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    UART1_IRQ => InterruptHandler<UART1>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // rc via SBUS
    let uart = p.UART1;
    let rx = p.PIN_5;
    let dma = p.DMA_CH1;

    let mut sbus_uart_config = Config::default();
    sbus_uart_config.baudrate = 100_000;
    sbus_uart_config.data_bits = DataBits::DataBits8;
    sbus_uart_config.stop_bits = StopBits::STOP2;
    sbus_uart_config.parity = Parity::ParityEven;
    sbus_uart_config.invert_rx = true;

    pub type UartRxSbusPeripheral =
        embassy_rp::uart::UartRx<'static, UART1, embassy_rp::uart::Async>;

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
                        false => defmt::info!("{}", packet.channels),
                        true => defmt::error!("Failsafe"),
                    }

                // Else if packet was not parsed, check if it took too long
                } else if parse_time.elapsed() > timeout {
                    defmt::error!("Parse timeout");
                }
            }
            Ok(Err(e)) => defmt::error!("Serial read {}", e),
            Err(_) => defmt::error!("Serial timeout"),
        }
    }
}
